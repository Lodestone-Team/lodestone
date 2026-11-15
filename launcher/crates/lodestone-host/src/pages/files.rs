//! An instance's Files tab (`components/FileViewer`): the folder's files, with
//! ticking, a clipboard (copy, cut, paste), new files and folders, rename, delete,
//! zip and unzip, upload (a dialog, or files dropped on the tab) and download (saved
//! where the user picks); and the text editor, with Save (⌘S / Ctrl+S) and Discard.
//!
//! Beyond the dashboard: opening another file or folder with unsaved changes asks
//! first, and changes left unsaved when the tab closes are kept (while the Host runs)
//! and come back when the file opens again, unless the file changed meanwhile. So are
//! the changes of a file that is moved, renamed or deleted (with a folder it is in).
//!
//! Pastes, deletes, uploads and downloads outlive the tab: switching tabs rebuilds the
//! page, and must not stop them partway. They say how they went through the Host's
//! toasts, and have the Files tab, open again or not, list the folder once done.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use gpui::{
    AnyElement, App, AppContext as _, AsyncApp, ClickEvent, Context, Entity, ExternalPaths,
    FocusHandle, Global, IntoElement, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement, Pixels, Point, Render, SharedString, Styled, Subscription, Task,
    UniformListScrollHandle, WeakEntity, Window, actions, anchored, deferred, div, img, prelude::*,
    uniform_list,
};
use launcher_core::cores::admin::{
    EventInner,
    events::{ProgressionEndValue, ProgressionEventInner},
    files::{
        ClientFile, FileType, MAX_READ_BYTES, UnzipOption, file_name, join, parent, separator,
    },
    models::InstancePermission,
};
use lodestone_ui::{
    components::{
        Button, ButtonVariant, Checkbox, Dialog, DialogSlot, Icon, Language, TextEditor,
        TextEditorEvent, TextInput, Toast,
        a11y::{Accessible, Role, Toggled},
        check_box, window_layer,
    },
    file_dialog::{self, ShowsError},
    i18n::{FormatMsg, FormatPlural, format},
    theme::{colors, fonts, ink, leading, px, radius, text, weights},
};

use super::console::{graphic, skeleton_code};
use crate::{
    i18n::{CommonMsg, ConsoleFilesMsg as M, ConsoleFilesPlural as P, t, tr, trn},
    state::{HostEvent, HostState},
};

mod find;

actions!(host_files, [SaveFile, CloseMenu, FindInFile]);

pub fn init(cx: &mut App) {
    let (save, find) = if cfg!(target_os = "macos") {
        ("cmd-s", "cmd-f")
    } else {
        ("ctrl-s", "ctrl-f")
    };
    cx.bind_keys([
        KeyBinding::new(save, SaveFile, Some("FilesPage")),
        KeyBinding::new(find, FindInFile, Some("FilesPage")),
        KeyBinding::new("escape", CloseMenu, Some("FilesMenu")),
    ]);
}

/// The folder each instance's Files tab showed last, and the list's width, while the
/// Host runs.
#[derive(Default)]
struct Remembered {
    paths: HashMap<String, String>,
    list_width: Option<f32>,
    /// Unsaved edits, by (instance, file): the text as read, and as edited.
    drafts: HashMap<(String, String), (String, String)>,
}

impl Global for Remembered {}

const LIST_MIN: f32 = 200.;
const LIST_DEFAULT: f32 = 300.;
const ROW_HEIGHT: f32 = 40.;

/// A file's state in the editor.
enum Content {
    Loading,
    /// The text as read (or last saved).
    Loaded(String),
    Error(String),
}

struct Opened {
    file: ClientFile,
    content: Content,
}

/// What the clipboard is for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClipAction {
    Copy,
    Cut,
}

/// What waits for the unsaved-changes dialog.
#[derive(Clone)]
enum Pending {
    OpenFile(ClientFile),
    OpenFolder(String),
    Close,
}

enum DialogKind {
    NewFile { dir: String },
    NewFolder { dir: String },
    Rename { path: String },
    Delete { files: Vec<ClientFile> },
    Unsaved { then: Pending, name: String },
}

struct FilesDialog {
    kind: DialogKind,
    focus: FocusHandle,
}

/// An open menu: the toolbar's, or the one a right-click opens (on a file, or on the
/// list's empty space).
enum Menu {
    Toolbar(Point<Pixels>),
    Context {
        at: Point<Pixels>,
        file: Option<ClientFile>,
    },
}

pub struct FilesPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
    sep: char,
    /// The instance's folder name, the breadcrumb's first part.
    root_name: String,
    /// The folder shown, relative to the instance (`.` at the top).
    pub path: String,
    pub files: Vec<ClientFile>,
    /// Loading the folder `path`; `files` are then the previous folder's, dimmed.
    loading: bool,
    list_error: Option<String>,
    loaded_once: bool,
    /// The ticked files' paths.
    ticked: Vec<String>,
    clipboard: Vec<ClientFile>,
    clip_action: ClipAction,
    opened: Option<Opened>,
    editor: Entity<TextEditor>,
    /// The editor's find bar, while open.
    find: Option<find::FindBar>,
    dirty: bool,
    saving: bool,
    list_scroll: UniformListScrollHandle,
    list_width: f32,
    /// While the divider is dragged: where the drag started, and the width then.
    resizing: Option<(Pixels, f32)>,
    menu: Option<Menu>,
    dialog: DialogSlot<FilesPage, FilesDialog>,
    name_input: Entity<TextInput>,
    dialog_error: Option<String>,
    dialog_busy: bool,
    /// Focus the name field at the next frame (a name dialog just opened).
    focus_name: bool,
    list_task: Option<Task<()>>,
    file_task: Option<Task<()>>,
    tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ShowsError for FilesPage {
    fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.toast(Toast::error(message), cx);
    }
}

impl FilesPage {
    pub fn new(state: Entity<HostState>, uuid: String, cx: &mut Context<Self>) -> Self {
        let (sep, root_name) = {
            let instance_path = state
                .read(cx)
                .instance(&uuid)
                .map(|i| i.path.clone())
                .unwrap_or_default();
            let sep = separator(&instance_path);
            let name = instance_path
                .trim_end_matches(sep)
                .rsplit(sep)
                .next()
                .unwrap_or_default()
                .to_string();
            (sep, name)
        };
        let remembered = cx.default_global::<Remembered>();
        let path = remembered
            .paths
            .get(&uuid)
            .cloned()
            .unwrap_or_else(|| ".".into());
        let list_width = remembered.list_width;
        let list_width = list_width
            .or(crate::prefs::Prefs::get(cx).file_list_width)
            .unwrap_or(LIST_DEFAULT)
            .max(LIST_MIN);
        let wrap = crate::prefs::Prefs::get(cx).editor_wrap;
        let editor = cx.new(|cx| {
            TextEditor::new(cx)
                .with_automation_id("file-editor")
                .with_soft_wrap(wrap)
        });
        let name_input = cx.new(|cx| TextInput::new(cx).with_automation_id("files-name"));
        let subscriptions = vec![
            cx.subscribe(&state, |this, _, event, cx| this.on_host_event(event, cx)),
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.subscribe(&editor, |this, editor, event, cx| match event {
                TextEditorEvent::Changed => {
                    let dirty = match this.opened.as_ref().map(|o| &o.content) {
                        Some(Content::Loaded(original)) => !editor.read(cx).text_equals(original),
                        _ => false,
                    };
                    if dirty != this.dirty {
                        this.dirty = dirty;
                        cx.notify();
                    }
                    // the matches moved with the edit
                    this.refind(false, cx);
                }
                TextEditorEvent::SoftWrapChanged(wrap) => {
                    let wrap = *wrap;
                    crate::prefs::Prefs::update(cx, |prefs| prefs.editor_wrap = wrap);
                    cx.notify();
                }
            }),
            // unsaved edits outlive the tab
            cx.on_release(|this, cx| this.keep_draft(cx)),
        ];
        let mut this = Self {
            state,
            uuid,
            sep,
            root_name,
            path,
            files: Vec::new(),
            loading: false,
            list_error: None,
            loaded_once: false,
            ticked: Vec::new(),
            clipboard: Vec::new(),
            clip_action: ClipAction::Copy,
            opened: None,
            find: None,
            editor,
            dirty: false,
            saving: false,
            list_scroll: UniformListScrollHandle::new(),
            list_width,
            resizing: None,
            menu: None,
            dialog: DialogSlot::new(|this: &mut FilesPage| &mut this.dialog),
            name_input,
            dialog_error: None,
            dialog_busy: false,
            focus_name: false,
            list_task: None,
            file_task: None,
            tasks: Vec::new(),
            _subscriptions: subscriptions,
        };
        if this.can_read(cx) {
            this.load(cx);
        }
        this
    }

    fn can_read(&self, cx: &App) -> bool {
        self.state
            .read(cx)
            .me
            .may(InstancePermission::ReadFiles, &self.uuid)
    }

    fn can_write(&self, cx: &App) -> bool {
        self.state
            .read(cx)
            .me
            .may(InstancePermission::WriteFiles, &self.uuid)
    }

    fn toast(&self, toast: Toast, cx: &mut App) {
        self.state.update(cx, |state, cx| state.toast(toast, cx));
    }

    fn spawn_task(&mut self, task: Task<()>) {
        self.tasks.retain(|task| !task.is_ready());
        self.tasks.push(task);
    }

    /// Keeps unsaved edits for when the file opens again.
    fn keep_draft(&mut self, cx: &mut App) {
        self.keep_draft_at(None, cx);
    }

    /// Keeps unsaved edits for when the file opens again, where it is and at
    /// `moved_to` (where a move or rename puts it), since the move may fail.
    fn keep_draft_at(&mut self, moved_to: Option<String>, cx: &mut App) {
        let Some(Opened {
            file,
            content: Content::Loaded(original),
        }) = &self.opened
        else {
            return;
        };
        if !self.dirty {
            return;
        }
        let text = self.editor.read(cx).text();
        let drafts = &mut cx.default_global::<Remembered>().drafts;
        for path in std::iter::once(file.path.clone()).chain(moved_to) {
            drafts.insert((self.uuid.clone(), path), (original.clone(), text.clone()));
        }
    }

    /// Closes the open file when a move or delete takes it away (it is one of the
    /// paths `taken`, or in a folder among them), keeping its unsaved edits. Each path
    /// comes with where it moves to, or `None` when it is deleted.
    fn close_if_taken(
        &mut self,
        taken: impl IntoIterator<Item = (String, Option<String>)>,
        cx: &mut Context<Self>,
    ) {
        let Some(opened) = self.opened.as_ref().map(|o| o.file.path.clone()) else {
            return;
        };
        let sep = self.sep;
        let Some((path, to)) = taken
            .into_iter()
            .find(|(path, _)| is_within(&opened, path, sep))
        else {
            return;
        };
        let moved = to.and_then(|to| moved_path(&opened, &path, &to, sep));
        self.keep_draft_at(moved, cx);
        self.close_file(cx);
    }

    // ---- The folder ----

    /// Fetches the folder `path` again.
    fn load(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        let admin = self.state.read(cx).admin.clone();
        let (uuid, path) = (self.uuid.clone(), self.path.clone());
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn({
                    let path = path.clone();
                    async move { admin.list_files(&uuid, &path).await }
                })
                .await;
            this.update(cx, |this, cx| {
                if this.path != path {
                    return;
                }
                this.loading = false;
                this.loaded_once = true;
                match result {
                    Ok(files) => {
                        this.files = files;
                        this.list_error = None;
                        // ticks of files that are gone go too
                        let paths: Vec<&str> = this.files.iter().map(|f| f.path.as_str()).collect();
                        this.ticked.retain(|p| paths.contains(&p.as_str()));
                    }
                    Err(err) => {
                        this.files.clear();
                        this.list_error = Some(err.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Opens the folder `path` (after asking about unsaved changes).
    fn open_folder(&mut self, path: String, cx: &mut Context<Self>) {
        if self.guard(Pending::OpenFolder(path.clone()), cx) {
            return;
        }
        self.go_to(path, cx);
    }

    fn go_to(&mut self, path: String, cx: &mut Context<Self>) {
        self.close_file(cx);
        self.ticked.clear();
        self.menu = None;
        if path != self.path {
            self.path = path.clone();
            cx.default_global::<Remembered>()
                .paths
                .insert(self.uuid.clone(), path);
            self.list_scroll = UniformListScrollHandle::new();
        }
        self.load(cx);
    }

    /// The core finished zipping or unzipping: lists the folder again. (The state says
    /// how it went, on any page, as the dashboard does.)
    fn on_host_event(&mut self, event: &HostEvent, cx: &mut Context<Self>) {
        if let HostEvent::FilesChanged(uuid) = event
            && *uuid == self.uuid
        {
            return self.load(cx);
        }
        let HostEvent::Core(event) = event else {
            return;
        };
        let EventInner::ProgressionEvent(progression) = &event.event_inner else {
            return;
        };
        if let ProgressionEventInner::ProgressionEnd {
            inner: Some(ProgressionEndValue::FsOperationCompleted { instance_uuid, .. }),
            ..
        } = &progression.progression_event_inner
            && *instance_uuid == self.uuid
        {
            self.load(cx);
        }
    }

    // ---- The editor ----

    /// Asks about unsaved changes before `then`; true when it asked.
    fn guard(&mut self, then: Pending, cx: &mut Context<Self>) -> bool {
        if !self.dirty {
            return false;
        }
        let name = self
            .opened
            .as_ref()
            .map(|o| o.file.name.clone())
            .unwrap_or_default();
        self.open_dialog(DialogKind::Unsaved { then, name }, cx);
        true
    }

    fn run(&mut self, pending: Pending, cx: &mut Context<Self>) {
        match pending {
            Pending::OpenFile(file) => self.open_file_now(file, cx),
            Pending::OpenFolder(path) => self.go_to(path, cx),
            Pending::Close => self.close_file(cx),
        }
    }

    fn open_file(&mut self, file: ClientFile, cx: &mut Context<Self>) {
        if self
            .opened
            .as_ref()
            .is_some_and(|o| o.file.path == file.path)
        {
            return;
        }
        if self.guard(Pending::OpenFile(file.clone()), cx) {
            return;
        }
        self.open_file_now(file, cx);
    }

    fn open_file_now(&mut self, file: ClientFile, cx: &mut Context<Self>) {
        self.menu = None;
        self.close_find(None, cx);
        self.dirty = false;
        self.editor.update(cx, |editor, cx| {
            editor.set_text("", cx);
            editor.set_language(Language::of(&file.name), cx);
        });
        if file.size.is_some_and(|size| size > MAX_READ_BYTES) {
            self.opened = Some(Opened {
                file,
                content: Content::Error(tr!(
                    M::FilesTooLarge,
                    size = format::bytes(MAX_READ_BYTES)
                )),
            });
            cx.notify();
            return;
        }
        self.opened = Some(Opened {
            file: file.clone(),
            content: Content::Loading,
        });
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let read_only = !self.can_write(cx);
        self.file_task = Some(cx.spawn(async move |this, cx| {
            let path = file.path.clone();
            let result = cx
                .background_spawn(async move { admin.read_file(&uuid, &path).await })
                .await;
            this.update(cx, |this, cx| {
                let Some(opened) = this.opened.as_mut().filter(|o| o.file.path == file.path) else {
                    return;
                };
                match result {
                    Ok(text) => {
                        let key = (this.uuid.clone(), file.path.clone());
                        let draft = cx
                            .default_global::<Remembered>()
                            .drafts
                            .remove(&key)
                            .filter(|(original, _)| *original == text);
                        let shown = draft.as_ref().map_or(text.as_str(), |(_, edited)| edited);
                        this.editor.update(cx, |editor, cx| {
                            editor.set_text(shown, cx);
                            editor.set_read_only(read_only, cx);
                        });
                        this.dirty = draft.is_some();
                        opened.content = Content::Loaded(text);
                    }
                    Err(err) => opened.content = Content::Error(err.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn close_file(&mut self, cx: &mut Context<Self>) {
        self.close_find(None, cx);
        self.opened = None;
        self.dirty = false;
        self.file_task = None;
        self.editor.update(cx, |editor, cx| editor.set_text("", cx));
        cx.notify();
    }

    /// Saves the editor's text; then runs `then`.
    fn save(&mut self, then: Option<Pending>, cx: &mut Context<Self>) {
        let Some(Opened {
            file,
            content: Content::Loaded(_),
        }) = &self.opened
        else {
            return;
        };
        if self.saving || !self.can_write(cx) {
            return;
        }
        let file = file.clone();
        let text = self.editor.read(cx).text();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        self.saving = true;
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn({
                    let (path, text) = (file.path.clone(), text.clone());
                    async move { admin.write_file(&uuid, &path, &text).await }
                })
                .await;
            this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(()) => {
                        if let Some(opened) =
                            this.opened.as_mut().filter(|o| o.file.path == file.path)
                        {
                            opened.content = Content::Loaded(text.clone());
                            this.dirty = !this.editor.read(cx).text_equals(&text);
                        }
                        // the list shows the new size and time
                        if let Some(entry) = this.files.iter_mut().find(|f| f.path == file.path) {
                            entry.size = Some(text.len() as u64);
                            entry.modification_time = Some(chrono::Utc::now().timestamp() as u64);
                        }
                        this.toast(Toast::success(t(CommonMsg::Saved)).key("files-saved"), cx);
                        if let Some(then) = then {
                            this.run(then, cx);
                        }
                    }
                    Err(err) => this.toast(
                        Toast::error(tr!(M::FilesSaveFailed, name = file.name, error = err)),
                        cx,
                    ),
                }
                cx.notify();
            })
            .ok();
        });
        self.spawn_task(task);
        cx.notify();
    }

    fn discard(&mut self, cx: &mut Context<Self>) {
        if let Some(Opened {
            content: Content::Loaded(original),
            ..
        }) = &self.opened
        {
            let original = original.clone();
            let read_only = !self.can_write(cx);
            self.editor.update(cx, |editor, cx| {
                editor.set_text(&original, cx);
                editor.set_read_only(read_only, cx);
            });
            self.dirty = false;
            cx.notify();
        }
    }

    fn on_save(&mut self, _: &SaveFile, _: &mut Window, cx: &mut Context<Self>) {
        if self.dirty {
            self.save(None, cx);
        }
    }

    // ---- Ticking and the clipboard ----

    fn is_ticked(&self, file: &ClientFile) -> bool {
        self.ticked.contains(&file.path)
    }

    fn tick(&mut self, file: &ClientFile, ticked: bool, cx: &mut Context<Self>) {
        self.ticked.retain(|p| *p != file.path);
        if ticked {
            self.ticked.push(file.path.clone());
        }
        cx.notify();
    }

    fn ticked_files(&self) -> Vec<ClientFile> {
        self.files
            .iter()
            .filter(|f| self.ticked.contains(&f.path))
            .cloned()
            .collect()
    }

    /// What a right-click on `file` acts on: the ticked files if it is one of them,
    /// else itself.
    fn selection_for(&self, file: &ClientFile) -> Vec<ClientFile> {
        if self.is_ticked(file) {
            self.ticked_files()
        } else {
            vec![file.clone()]
        }
    }

    fn put_on_clipboard(
        &mut self,
        files: Vec<ClientFile>,
        action: ClipAction,
        cx: &mut Context<Self>,
    ) {
        if files.is_empty() {
            return;
        }
        let count = files.len();
        self.clipboard = files;
        self.clip_action = action;
        self.ticked.clear();
        let message = match action {
            ClipAction::Copy => trn!(P::FilesCopiedToClipboard, count),
            ClipAction::Cut => trn!(P::FilesCutToClipboard, count),
        };
        self.toast(Toast::info(message).key("files-clipboard"), cx);
        cx.notify();
    }

    /// Pastes the clipboard into `dir`: copies, or moves what was cut. It goes on
    /// when the page closes (see the module's notes).
    fn paste(&mut self, dir: String, cx: &mut Context<Self>) {
        if self.clipboard.is_empty() {
            return;
        }
        let files = std::mem::take(&mut self.clipboard);
        let action = self.clip_action;
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let sep = self.sep;
        // a moved file can't stay open
        if action == ClipAction::Cut {
            let taken = files
                .iter()
                .map(|file| (file.path.clone(), Some(join(&dir, &file.name, sep))));
            self.close_if_taken(taken.collect::<Vec<_>>(), cx);
        }
        let state = self.state.downgrade();
        cx.spawn(async move |_, cx| {
            let failures = cx
                .background_spawn({
                    let uuid = uuid.clone();
                    async move {
                        let mut failures = Vec::new();
                        match action {
                            ClipAction::Copy => {
                                let sources: Vec<String> =
                                    files.iter().map(|f| f.path.clone()).collect();
                                if let Err(err) = admin.copy_paths(&uuid, &sources, &dir).await {
                                    failures.push(err.to_string());
                                }
                            }
                            ClipAction::Cut => {
                                for file in &files {
                                    let dest = join(&dir, &file.name, sep);
                                    if let Err(err) =
                                        admin.move_path(&uuid, &file.path, &dest).await
                                    {
                                        failures.push(tr!(
                                            M::FilesMoveFailed,
                                            name = file.name,
                                            error = err
                                        ));
                                    }
                                }
                            }
                        }
                        failures
                    }
                })
                .await;
            for failure in failures {
                toast_later(&state, Toast::error(failure), cx);
            }
            files_changed(&state, &uuid, cx);
        })
        .detach();
        cx.notify();
    }

    // ---- Zip, unzip, delete ----

    fn zip(&mut self, files: Vec<ClientFile>, cx: &mut Context<Self>) {
        if files.is_empty() {
            return;
        }
        let archive = zip_name(&files);
        let dest = join(&self.path, &archive, self.sep);
        let targets: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
        let count = targets.len();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        self.ticked.clear();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.zip_paths(&uuid, &targets, &dest).await })
                .await;
            this.update(cx, |this, cx| match result {
                Ok(()) => this.toast(Toast::info(trn!(P::FilesZipping, count)), cx),
                Err(err) => this.toast(Toast::error(tr!(M::FilesZipFailed, error = err)), cx),
            })
            .ok();
        });
        self.spawn_task(task);
        cx.notify();
    }

    fn unzip(&mut self, file: ClientFile, option: UnzipOption, cx: &mut Context<Self>) {
        if file.file_type != FileType::File {
            return self.toast(Toast::error(t(M::FilesOnlyFilesUnzip)), cx);
        }
        if !file.is_archive() {
            return self.toast(Toast::error(t(M::FilesUnsupportedZip)), cx);
        }
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let path = file.path.clone();
            let result = cx
                .background_spawn(async move { admin.unzip(&uuid, &path, &option).await })
                .await;
            this.update(cx, |this, cx| match result {
                Ok(()) => this.toast(Toast::info(tr!(M::FilesUnzipping, name = file.name)), cx),
                Err(err) => this.toast(
                    Toast::error(tr!(M::FilesUnzipFailed, name = file.name, error = err)),
                    cx,
                ),
            })
            .ok();
        });
        self.spawn_task(task);
    }

    /// Deletes files and folders, one by one. It goes on when the page closes (see
    /// the module's notes).
    fn delete(&mut self, files: Vec<ClientFile>, cx: &mut Context<Self>) {
        // the edits come back if the delete fails and the file opens again
        self.close_if_taken(
            files
                .iter()
                .map(|f| (f.path.clone(), None))
                .collect::<Vec<_>>(),
            cx,
        );
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let state = self.state.downgrade();
        cx.spawn(async move |this, cx| {
            // one by one, as the dashboard does, each with its own toast
            for file in files {
                let result = cx
                    .background_spawn({
                        let (admin, uuid, path, dir) = (
                            admin.clone(),
                            uuid.clone(),
                            file.path.clone(),
                            file.is_dir(),
                        );
                        async move { admin.delete_path(&uuid, &path, dir).await }
                    })
                    .await;
                let toast = match result {
                    Ok(()) if file.is_dir() => {
                        Toast::success(tr!(M::FilesDeletedFolder, path = file.path))
                    }
                    Ok(()) => Toast::success(tr!(M::FilesDeletedFile, name = file.name)),
                    Err(err) => Toast::error(err.to_string()),
                };
                toast_later(&state, toast, cx);
                this.update(cx, |this, cx| {
                    this.ticked.retain(|p| *p != file.path);
                    cx.notify();
                })
                .ok();
            }
            files_changed(&state, &uuid, cx);
        })
        .detach();
    }

    // ---- Upload and download ----

    fn choose_uploads(&mut self, cx: &mut Context<Self>) {
        let dialog = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(t(M::FilesUpload).into()),
        });
        let task = cx.spawn(async move |this, cx| {
            if let Some(paths) = file_dialog::chosen(dialog, &this, cx).await {
                this.update(cx, |this, cx| this.upload(paths, cx)).ok();
            }
        });
        self.spawn_task(task);
    }

    /// Uploads files from this computer into the folder shown, streamed from disk.
    /// Folders are refused. It goes on when the page closes (see the module's notes).
    fn upload(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.is_empty() || !self.can_write(cx) {
            return;
        }
        if paths.iter().any(|p| p.is_dir()) {
            return self.toast(Toast::error(t(M::FilesOnlyFilesUpload)), cx);
        }
        let count = paths.len();
        self.toast(Toast::info(trn!(P::FilesUploading, count)), cx);
        let admin = self.state.read(cx).admin.clone();
        let (uuid, dir) = (self.uuid.clone(), self.path.clone());
        let state = self.state.downgrade();
        cx.spawn(async move |_, cx| {
            let result = cx
                .background_spawn({
                    let uuid = uuid.clone();
                    async move { admin.upload_paths(&uuid, &dir, &paths).await }
                })
                .await;
            if let Err(err) = result {
                toast_later(&state, Toast::error(err.to_string()), cx);
            }
            files_changed(&state, &uuid, cx);
        })
        .detach();
    }

    /// Downloads files: one is saved where the user says, several into a folder they
    /// pick, each under a name nothing there has yet. A folder comes as a zip, made by
    /// the core. Each is written next to where it goes and only put there once
    /// complete, so a failed download leaves no partial file and harms no file the
    /// user had. Downloads go on when the page closes (see the module's notes).
    fn download(&mut self, files: Vec<ClientFile>, cx: &mut Context<Self>) {
        if files.is_empty() {
            return;
        }
        let downloads = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let single = files.len() == 1;
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let state = self.state.downgrade();
        cx.spawn(async move |this, cx| {
            let targets: Vec<(ClientFile, PathBuf)> = if single {
                let file = files[0].clone();
                let name = download_name(&file);
                // the save dialog asks before replacing a file
                let dialog = cx.update(|cx| cx.prompt_for_new_path(&downloads, Some(&name)));
                let Some(out) = file_dialog::chosen(dialog, &this, cx).await else {
                    return;
                };
                vec![(file, out)]
            } else {
                let dialog = cx.update(|cx| {
                    cx.prompt_for_paths(gpui::PathPromptOptions {
                        files: false,
                        directories: true,
                        multiple: false,
                        prompt: Some(t(M::FilesDownloadSelected).into()),
                    })
                });
                let Some(dir) = file_dialog::chosen(dialog, &this, cx)
                    .await
                    .and_then(|dirs| dirs.into_iter().next())
                else {
                    return;
                };
                let names: Vec<String> = files.iter().map(download_name).collect();
                let outs = download_paths(&dir, &names, |path| path.exists());
                files.into_iter().zip(outs).collect()
            };
            for (file, out) in targets {
                if file.is_dir() {
                    toast_later(&state, Toast::info(t(M::FilesZippingForDownload)), cx);
                }
                this.update(cx, |this, cx| {
                    this.ticked.retain(|p| *p != file.path);
                    cx.notify();
                })
                .ok();
                let result = cx
                    .background_spawn({
                        let (admin, uuid, file, out) =
                            (admin.clone(), uuid.clone(), file.clone(), out.clone());
                        async move { admin.download_to(&uuid, &file, &out).await }
                    })
                    .await;
                let toast = match result {
                    Ok(_) => {
                        let shown = out.clone();
                        Toast::success(tr!(M::FilesDownloaded, name = file.name))
                            .action(t(M::FilesShowInFolder), move |_, cx| cx.reveal_path(&shown))
                    }
                    Err(err) => {
                        Toast::error(tr!(M::FilesDownloadFailed, name = file.name, error = err))
                    }
                };
                toast_later(&state, toast, cx);
            }
        })
        .detach();
    }

    // ---- Dialogs ----

    fn open_dialog(&mut self, kind: DialogKind, cx: &mut Context<Self>) {
        self.menu = None;
        self.dialog_error = None;
        self.dialog_busy = false;
        let (text, placeholder) = match &kind {
            DialogKind::NewFile { .. } => (String::new(), Some(M::FilesNameFilePlaceholder)),
            DialogKind::NewFolder { .. } => (String::new(), Some(M::FilesNameFolderPlaceholder)),
            DialogKind::Rename { path } => (
                file_name(path, self.sep).to_string(),
                Some(M::FilesNameFilePlaceholder),
            ),
            _ => (String::new(), None),
        };
        let has_input = placeholder.is_some();
        self.name_input.update(cx, |input, cx| {
            input.set_text(text, cx);
            if let Some(placeholder) = placeholder {
                input.set_placeholder(placeholder, cx);
            }
        });
        self.focus_name = has_input;
        let focus = cx.focus_handle();
        self.dialog.open(FilesDialog { kind, focus }, cx);
    }

    fn close_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.dialog_busy {
            self.dialog.close(cx);
        }
    }

    /// The name dialog's button: makes the file or folder, or renames.
    fn submit_name(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialog.current() else {
            return;
        };
        let name = self.name_input.read(cx).text().trim().to_string();
        if name.is_empty() {
            self.dialog_error = Some(t(M::FilesNameRequired).into());
            cx.notify();
            return;
        }
        let sep = self.sep;
        let (target, action): (String, u8) = match &dialog.kind {
            DialogKind::NewFile { dir } => (join(dir, &name, sep), 0),
            DialogKind::NewFolder { dir } => (join(dir, &name, sep), 1),
            DialogKind::Rename { path } => (path.clone(), 2),
            _ => return,
        };
        // a new file by the name of one there would empty it: the folder shown is
        // checked at once, and any folder again just before it is made (below)
        let dir = match &dialog.kind {
            DialogKind::NewFile { dir } | DialogKind::NewFolder { dir } => Some(dir.clone()),
            _ => None,
        };
        if dir.as_deref() == Some(self.path.as_str()) && name_taken(&self.files, &name) {
            self.dialog_error = Some(tr!(M::FilesNameTaken, name = name));
            cx.notify();
            return;
        }
        let renamed = {
            // the same folder, the new name
            let at = target.rfind(sep).map_or(0, |i| i + 1);
            format!("{}{name}", &target[..at])
        };
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        self.dialog_busy = true;
        self.dialog_error = None;
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn({
                    let (target, renamed, name) = (target.clone(), renamed.clone(), name.clone());
                    // `Err(None)`: the name is taken
                    async move {
                        if let Some(dir) = &dir {
                            let files = admin.list_files(&uuid, dir).await.map_err(Some)?;
                            if name_taken(&files, &name) {
                                return Err(None);
                            }
                        }
                        match action {
                            0 => admin.new_file(&uuid, &target).await,
                            1 => admin.new_folder(&uuid, &target).await,
                            _ => admin.move_path(&uuid, &target, &renamed).await,
                        }
                        .map_err(Some)
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.dialog_busy = false;
                match result {
                    Ok(()) => {
                        this.dialog.close(cx);
                        if action == 2 {
                            // the renamed file (or one in the renamed folder) closes,
                            // and its unsaved edits wait for it under its new name
                            this.close_if_taken([(target.clone(), Some(renamed.clone()))], cx);
                        }
                        this.load(cx);
                    }
                    Err(None) => {
                        this.dialog_error = Some(tr!(M::FilesNameTaken, name = name));
                    }
                    Err(Some(err)) => {
                        if action == 2 {
                            let name = file_name(&target, sep).to_string();
                            this.toast(
                                Toast::error(tr!(M::FilesMoveFailed, name = name, error = err)),
                                cx,
                            );
                        }
                        this.dialog_error = Some(err.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.spawn_task(task);
        cx.notify();
    }

    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        let files = match self.dialog.current() {
            Some(FilesDialog {
                kind: DialogKind::Delete { files },
                ..
            }) => files.clone(),
            _ => return,
        };
        self.dialog.close(cx);
        self.delete(files, cx);
    }

    /// The unsaved-changes dialog's buttons.
    fn resolve_unsaved(&mut self, save: bool, cx: &mut Context<Self>) {
        let then = match self.dialog.current() {
            Some(FilesDialog {
                kind: DialogKind::Unsaved { then, .. },
                ..
            }) => then.clone(),
            _ => return,
        };
        self.dialog.close(cx);
        if save {
            self.save(Some(then), cx);
        } else {
            self.dirty = false;
            self.run(then, cx);
        }
    }

    fn render_dialog(&self, dialog: &FilesDialog, cx: &mut Context<Self>) -> AnyElement {
        let close = cx.listener(|this, _: &ClickEvent, _, cx| this.close_dialog(cx));
        let on_close = {
            let page = cx.entity().downgrade();
            move |_: &mut Window, cx: &mut App| {
                page.update(cx, |this, cx| this.close_dialog(cx)).ok();
            }
        };
        let body_text = |text: SharedString| {
            div()
                .text_size(text::MEDIUM)
                .line_height(lodestone_ui::theme::line_height(
                    text::MEDIUM,
                    leading::NORMAL,
                ))
                .text_color(ink::secondary())
                .child(text)
        };
        match &dialog.kind {
            DialogKind::NewFile { .. }
            | DialogKind::NewFolder { .. }
            | DialogKind::Rename { .. } => {
                let (title, label, submit) = match &dialog.kind {
                    DialogKind::NewFile { .. } => {
                        (M::FilesNewFile, M::FilesNameFile, M::FilesCreateFile)
                    }
                    DialogKind::NewFolder { .. } => {
                        (M::FilesNewFolder, M::FilesNameFolder, M::FilesCreateFolder)
                    }
                    _ => (M::FilesRenameFile, M::FilesNameFile, M::FilesRenameFile),
                };
                Dialog::new("files-name-dialog", t(title), dialog.focus.clone())
                    .on_close(on_close)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(text::MEDIUM)
                                    .font_weight(weights::MEDIUM.face())
                                    .text_color(ink::text())
                                    .child(t(label)),
                            )
                            .child(
                                div()
                                    .on_key_down(cx.listener(
                                        |this, event: &gpui::KeyDownEvent, _, cx| {
                                            if event.keystroke.key == "enter" {
                                                this.submit_name(cx);
                                            }
                                        },
                                    ))
                                    .child(self.name_input.clone()),
                            )
                            .when_some(self.dialog_error.clone(), |this, error| {
                                this.child(
                                    div()
                                        .text_size(text::SMALL)
                                        .text_color(colors::RED_200)
                                        .child(error),
                                )
                            }),
                    )
                    .footer(Button::new("files-name-cancel", t(CommonMsg::Cancel)).on_click(close))
                    .footer(
                        Button::new("files-name-submit", t(submit))
                            .variant(ButtonVariant::Primary)
                            .loading(self.dialog_busy)
                            .disabled(self.dialog_busy)
                            .on_click(cx.listener(|this, _, _, cx| this.submit_name(cx))),
                    )
                    .into_any_element()
            }
            DialogKind::Delete { files } => {
                let names = files.iter().map(|file| {
                    let suffix = match file.file_type {
                        FileType::Directory => format!(" {}", t(M::FilesDeleteDirectory)),
                        FileType::Unknown => format!(" {}", t(M::FilesDeleteUnknownType)),
                        FileType::File => String::new(),
                    };
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(6.))
                        .child("•")
                        .child(format!("{}{suffix}", file.name))
                });
                Dialog::new(
                    "files-delete-dialog",
                    trn!(P::FilesDeleteTitle, files.len()),
                    dialog.focus.clone(),
                )
                .on_close(on_close)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(body_text(t(M::FilesDeleteMessage).into()))
                        .child(
                            div()
                                .id("files-delete-list")
                                .max_h(px(240.))
                                .overflow_y_scroll()
                                .text_size(text::MEDIUM)
                                .text_color(ink::text())
                                .children(names),
                        ),
                )
                .footer(Button::new("files-delete-cancel", t(CommonMsg::Cancel)).on_click(close))
                .footer(
                    Button::new("files-delete-confirm", t(CommonMsg::Delete))
                        .variant(ButtonVariant::Danger)
                        .on_click(cx.listener(|this, _, _, cx| this.confirm_delete(cx))),
                )
                .into_any_element()
            }
            DialogKind::Unsaved { name, .. } => Dialog::new(
                "files-unsaved-dialog",
                t(CommonMsg::UnsavedChanges),
                dialog.focus.clone(),
            )
            .on_close(on_close)
            .child(body_text(tr!(M::FilesSaveChanges, name = name).into()))
            .footer_start(
                Button::new("files-unsaved-keep", t(CommonMsg::KeepEditing)).on_click(close),
            )
            .footer(
                Button::new("files-unsaved-discard", t(CommonMsg::Discard))
                    .variant(ButtonVariant::Danger)
                    .on_click(cx.listener(|this, _, _, cx| this.resolve_unsaved(false, cx))),
            )
            .footer(
                Button::new("files-unsaved-save", t(CommonMsg::Save))
                    .variant(ButtonVariant::Primary)
                    .disabled(!self.can_write(cx))
                    .on_click(cx.listener(|this, _, _, cx| this.resolve_unsaved(true, cx))),
            )
            .into_any_element(),
        }
    }

    // ---- Menus ----

    fn close_menu(&mut self, _: &CloseMenu, _: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    /// The toolbar's Add/Remove menu.
    fn toolbar_menu(&self, cx: &mut Context<Self>) -> Vec<Vec<AnyElement>> {
        let can_write = self.can_write(cx);
        let can_read = self.can_read(cx);
        let ticked = self.ticked.len();
        let all_ticked = !self.files.is_empty() && ticked == self.files.len();
        vec![
            vec![
                menu_item(
                    "files-select-all",
                    if all_ticked {
                        t(M::FilesDeselectAll)
                    } else {
                        t(M::FilesSelectAll)
                    },
                    Some(Icon::ListCheck),
                    self.files.is_empty(),
                    false,
                    cx.listener(move |this, _, _, cx| {
                        this.ticked = if all_ticked {
                            Vec::new()
                        } else {
                            this.files.iter().map(|f| f.path.clone()).collect()
                        };
                        this.menu = None;
                        cx.notify();
                    }),
                ),
                menu_item(
                    "files-cut-selected",
                    t(M::FilesCutSelected),
                    Some(Icon::Scissors),
                    ticked == 0 || !can_write,
                    false,
                    cx.listener(|this, _, _, cx| {
                        this.menu = None;
                        let files = this.ticked_files();
                        this.put_on_clipboard(files, ClipAction::Cut, cx);
                    }),
                ),
                menu_item(
                    "files-download-selected",
                    t(M::FilesDownloadSelected),
                    Some(Icon::Download),
                    ticked == 0 || !can_read,
                    false,
                    cx.listener(|this, _, _, cx| {
                        this.menu = None;
                        let files = this.ticked_files();
                        this.download(files, cx);
                    }),
                ),
                menu_item(
                    "files-unarchive-selected",
                    t(M::FilesUnarchiveSelected),
                    Some(Icon::FileZipper),
                    ticked != 1 || !can_write,
                    false,
                    cx.listener(|this, _, _, cx| {
                        this.menu = None;
                        if let Some(file) = this.ticked_files().pop() {
                            this.ticked.clear();
                            this.unzip(file, UnzipOption::Smart, cx);
                        }
                        cx.notify();
                    }),
                ),
            ],
            vec![
                menu_item(
                    "files-new-file",
                    t(M::FilesNewFile),
                    Some(Icon::FileCirclePlus),
                    !can_write,
                    false,
                    cx.listener(|this, _, _, cx| {
                        let dir = this.path.clone();
                        this.open_dialog(DialogKind::NewFile { dir }, cx);
                    }),
                ),
                menu_item(
                    "files-new-folder",
                    t(M::FilesNewFolder),
                    Some(Icon::FolderPlus),
                    !can_write,
                    false,
                    cx.listener(|this, _, _, cx| {
                        let dir = this.path.clone();
                        this.open_dialog(DialogKind::NewFolder { dir }, cx);
                    }),
                ),
            ],
            vec![menu_item(
                "files-delete-selected",
                t(M::FilesDeleteSelected),
                Some(Icon::TrashCan),
                ticked == 0 || !can_write,
                true,
                cx.listener(|this, _, _, cx| {
                    let files = this.ticked_files();
                    this.open_dialog(DialogKind::Delete { files }, cx);
                }),
            )],
        ]
    }

    /// The right-click menu (`FileContextMenu.tsx`).
    fn context_menu(
        &self,
        file: Option<ClientFile>,
        cx: &mut Context<Self>,
    ) -> Vec<Vec<AnyElement>> {
        let can_write = self.can_write(cx);
        let selection = file
            .as_ref()
            .map(|f| self.selection_for(f))
            .unwrap_or_default();
        let target_dir = match &file {
            Some(f) if f.is_dir() => f.path.clone(),
            _ => self.path.clone(),
        };
        let ticked = self.ticked_files();
        let zip_label = match (&file, ticked.len()) {
            (None, 0) => t(M::FilesZip).to_string(),
            (Some(f), 0) => tr!(M::FilesZipTo, archive = format!("{}.zip", f.name)),
            (_, 1) => tr!(
                M::FilesZipFile,
                name = ticked[0].name,
                archive = format!("{}.zip", ticked[0].name)
            ),
            (_, n) => trn!(P::FilesZipFiles, n, archive = "Archive.zip"),
        };
        let zip_targets = if ticked.is_empty() {
            file.iter().cloned().collect()
        } else {
            ticked.clone()
        };
        let paste_label = match &file {
            Some(f) if f.is_dir() => tr!(M::FilesPasteInto, name = f.name),
            _ => t(M::FilesPaste).to_string(),
        };
        let mut groups = vec![
            vec![
                menu_item(
                    "files-copy",
                    t(CommonMsg::Copy),
                    None,
                    file.is_none(),
                    false,
                    cx.listener({
                        let selection = selection.clone();
                        move |this, _, _, cx| {
                            this.menu = None;
                            this.put_on_clipboard(selection.clone(), ClipAction::Copy, cx);
                        }
                    }),
                ),
                menu_item(
                    "files-cut",
                    t(M::FilesCut),
                    None,
                    file.is_none() || !can_write,
                    false,
                    cx.listener({
                        let selection = selection.clone();
                        move |this, _, _, cx| {
                            this.menu = None;
                            this.put_on_clipboard(selection.clone(), ClipAction::Cut, cx);
                        }
                    }),
                ),
                menu_item(
                    "files-paste",
                    paste_label,
                    None,
                    self.clipboard.is_empty() || !can_write,
                    false,
                    cx.listener({
                        let dir = target_dir.clone();
                        move |this, _, _, cx| {
                            this.menu = None;
                            this.paste(dir.clone(), cx);
                        }
                    }),
                ),
            ],
            vec![
                menu_item(
                    "files-rename",
                    t(CommonMsg::Rename),
                    None,
                    file.is_none() || !can_write,
                    false,
                    cx.listener({
                        let path = file.as_ref().map(|f| f.path.clone()).unwrap_or_default();
                        move |this, _, _, cx| {
                            let path = path.clone();
                            this.open_dialog(DialogKind::Rename { path }, cx);
                        }
                    }),
                ),
                menu_item(
                    "files-delete",
                    t(CommonMsg::Delete),
                    None,
                    file.is_none() || !can_write,
                    true,
                    cx.listener({
                        let selection = selection.clone();
                        move |this, _, _, cx| {
                            let files = selection.clone();
                            this.open_dialog(DialogKind::Delete { files }, cx);
                        }
                    }),
                ),
            ],
        ];
        let mut zip_group = vec![menu_item(
            "files-zip",
            zip_label,
            None,
            zip_targets.is_empty() || !can_write,
            false,
            cx.listener(move |this, _, _, cx| {
                this.menu = None;
                this.zip(zip_targets.clone(), cx);
            }),
        )];
        if let Some(file) = file.as_ref().filter(|f| f.is_archive()) {
            for (id, label, option) in [
                (
                    "files-unzip-here",
                    t(M::FilesUnzipHere).to_string(),
                    UnzipOption::Normal,
                ),
                (
                    "files-unzip-smart",
                    t(M::FilesUnzipHereSmart).to_string(),
                    UnzipOption::Smart,
                ),
                (
                    "files-unzip-to",
                    tr!(
                        M::FilesUnzipTo,
                        name = format!("{}{}", file.file_stem, self.sep)
                    ),
                    UnzipOption::ToDirectoryWithFileName,
                ),
            ] {
                let file = file.clone();
                zip_group.push(menu_item(
                    id,
                    label,
                    None,
                    !can_write,
                    false,
                    cx.listener(move |this, _, _, cx| {
                        this.menu = None;
                        this.unzip(file.clone(), option.clone(), cx);
                        cx.notify();
                    }),
                ));
            }
        }
        groups.push(zip_group);
        groups.push(vec![
            menu_item(
                "files-context-new-folder",
                t(M::FilesNewFolder),
                None,
                !can_write,
                false,
                cx.listener({
                    let dir = target_dir.clone();
                    move |this, _, _, cx| {
                        let dir = dir.clone();
                        this.open_dialog(DialogKind::NewFolder { dir }, cx);
                    }
                }),
            ),
            menu_item(
                "files-context-new-file",
                t(M::FilesNewFile),
                None,
                !can_write,
                false,
                cx.listener(move |this, _, _, cx| {
                    let dir = target_dir.clone();
                    this.open_dialog(DialogKind::NewFile { dir }, cx);
                }),
            ),
        ]);
        groups
    }

    // ---- Drawing ----

    fn breadcrumb(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let opened = self.opened.as_ref().map(|o| o.file.name.clone());
        let at_top = self.path == "." || self.path.is_empty();
        let sep = self.sep;
        let mut parts: Vec<(String, String)> = Vec::new();
        let mut so_far = String::new();
        for part in self.path.split(sep) {
            so_far = if so_far.is_empty() {
                part.to_string()
            } else {
                format!("{so_far}{sep}{part}")
            };
            if part.is_empty() || part == "." {
                continue;
            }
            parts.push((part.to_string(), so_far.clone()));
        }
        let link = |id: SharedString,
                    label: String,
                    target: String,
                    active: bool,
                    cx: &mut Context<Self>| {
            div()
                .id(id.clone())
                .flex_none()
                .when(active, |this| {
                    this.accessible(Role::Link, id, label.clone())
                        .cursor_pointer()
                        .text_color(colors::BLUE_200)
                        .hover(|style| style.underline())
                        .on_press(
                            cx.listener(move |this, _, _, cx| this.open_folder(target.clone(), cx)),
                        )
                })
                .child(label)
        };
        let count = parts.len();
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .min_w_0()
            .flex_1()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_size(text::MEDIUM)
            .font_weight(weights::MEDIUM.face())
            .text_color(colors::GRAY_300)
            .child(link(
                "files-crumb-root".into(),
                self.root_name.clone(),
                ".".into(),
                !at_top || opened.is_some(),
                cx,
            ));
        for (i, (label, target)) in parts.into_iter().enumerate() {
            row = row
                .child(div().flex_none().child(sep.to_string()))
                .child(link(
                    format!("files-crumb-{i}").into(),
                    label,
                    target,
                    i + 1 < count || opened.is_some(),
                    cx,
                ));
        }
        if let Some(name) = opened {
            row = row
                .child(div().flex_none().child(sep.to_string()))
                .child(div().truncate().child(name));
        }
        row
    }

    /// For the development screenshots: `server.properties` open, `eula.txt` ticked
    /// and its right-click menu shown, or (`LODESTONE_HOST_DEMO=large`) a large file.
    #[cfg(feature = "screenshot")]
    pub fn demo(&mut self, cx: &mut Context<Self>) {
        let large = std::env::var("LODESTONE_HOST_DEMO").as_deref() == Ok("large");
        if large {
            let file = ClientFile {
                name: "big.toml".into(),
                file_stem: "big".into(),
                extension: Some("toml".into()),
                path: join("config", "big.toml", self.sep),
                size: None,
                creation_time: None,
                modification_time: None,
                file_type: FileType::File,
            };
            self.open_file_now(file, cx);
            return;
        }
        let files = self.files.clone();
        let find = |name: &str| files.iter().find(|f| f.name == name).cloned();
        if let Some(file) = find("server.properties") {
            self.open_file_now(file, cx);
        }
        if let Some(eula) = find("eula.txt") {
            self.ticked.push(eula.path.clone());
            self.menu = Some(Menu::Context {
                at: gpui::point(px(640.), px(560.)),
                file: Some(eula),
            });
        }
        if let Some(world) = find("world") {
            self.clipboard = vec![world];
            self.clip_action = ClipAction::Cut;
        }
        cx.notify();
    }

    fn row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let at_top = self.path == "." || self.path.is_empty();
        let base = || {
            div()
                .h(px(ROW_HEIGHT))
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(16.))
                .px(px(16.))
                .border_b_1()
                .border_color(colors::GRAY_FADED.opacity(30))
                .text_size(text::MEDIUM)
                .font_weight(weights::MEDIUM.face())
                .whitespace_nowrap()
        };
        let mut ix = ix;
        if !at_top {
            if ix == 0 {
                let target = parent(&self.path, self.sep);
                return base()
                    .id("files-parent")
                    .accessible(Role::Link, "files-parent", "..")
                    .bg(colors::GRAY_800)
                    .cursor_pointer()
                    .hover(|style| style.bg(colors::GRAY_700).text_color(colors::BLUE_200))
                    .on_press(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.open_folder(target.clone(), cx)
                    }))
                    .child("..")
                    .into_any_element();
            }
            ix -= 1;
        }
        let Some(file) = self.files.get(ix) else {
            // the status row: loading, an error, or an empty folder
            let message: AnyElement = if self.loading && !self.loaded_once {
                div().w(px(160.)).child(skeleton_code(1)).into_any_element()
            } else if let Some(error) = &self.list_error {
                div()
                    .text_color(colors::GRAY_400)
                    .child(error.clone())
                    .into_any_element()
            } else {
                div()
                    .text_color(colors::GRAY_400)
                    .child(t(M::FilesEmpty))
                    .into_any_element()
            };
            return base().child(message).into_any_element();
        };
        let opened = self
            .opened
            .as_ref()
            .is_some_and(|o| o.file.path == file.path);
        let ticked = self.is_ticked(file);
        let cut = self.clip_action == ClipAction::Cut
            && self.clipboard.iter().any(|f| f.path == file.path);
        let stale = self.loading && self.loaded_once;
        let width = self.list_width;
        let show_time = self.opened.is_none() || width >= 360.;
        let show_size = self.opened.is_none() || width >= 440.;
        let time = file
            .modification_time
            .or(file.creation_time)
            .map_or_else(|| t(M::FilesUnknownTime).to_string(), time_label);
        let size = (!file.is_dir())
            .then(|| file.size.map(format::bytes))
            .flatten()
            .unwrap_or_default();
        let id_part = ix;
        let for_tick = file.clone();
        let for_open = file.clone();
        let for_menu = file.clone();
        base()
            .id(("files-row", id_part))
            .bg(if opened {
                colors::GRAY_900
            } else if ticked {
                colors::GRAY_700
            } else {
                colors::GRAY_800
            })
            .hover(|style| style.bg(colors::GRAY_700))
            .when(stale, |this| this.opacity(0.5))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.menu = Some(Menu::Context {
                        at: event.position,
                        file: Some(for_menu.clone()),
                    });
                    cx.notify();
                }),
            )
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id(("files-tick", id_part))
                    .accessible(
                        Role::CheckBox,
                        format!("files-tick-{id_part}"),
                        tr!(M::FilesSelect, name = file.name),
                    )
                    .aria_toggled(if ticked {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                    .flex_none()
                    .p(px(4.))
                    .rounded_full()
                    .cursor_pointer()
                    .hover(|style| style.bg(colors::GRAY_FADED.opacity(30)))
                    .child(check_box(ticked, false))
                    .on_press(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.tick(&for_tick, !ticked, cx)
                    })),
            )
            .child(file_icon(file))
            .child(
                div()
                    .id(("files-name", id_part))
                    .accessible(
                        Role::Link,
                        format!("files-name-{id_part}"),
                        file.name.clone(),
                    )
                    .min_w_0()
                    .truncate()
                    .cursor_pointer()
                    .when(opened, |this| this.italic())
                    .text_color(if cut {
                        colors::GRAY_300.opacity(60)
                    } else {
                        colors::GRAY_300
                    })
                    .hover(|style| style.underline().text_color(colors::BLUE_200))
                    .child(file.name.clone())
                    .on_press(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if for_open.is_dir() {
                            this.open_folder(for_open.path.clone(), cx);
                        } else {
                            this.open_file(for_open.clone(), cx);
                        }
                    })),
            )
            .child(div().flex_1())
            .when(show_time, |this| {
                this.child(
                    div()
                        .flex_none()
                        .min_w(px(72.))
                        .text_right()
                        .text_color(lodestone_ui::theme::ink::muted())
                        .child(time),
                )
            })
            .when(show_size, |this| {
                this.child(
                    div()
                        .flex_none()
                        .min_w(px(72.))
                        .text_right()
                        .text_color(lodestone_ui::theme::ink::muted())
                        .child(size),
                )
            })
            .into_any_element()
    }

    fn row_count(&self) -> usize {
        let at_top = self.path == "." || self.path.is_empty();
        let status = (self.files.is_empty()) as usize;
        usize::from(!at_top) + self.files.len() + status
    }

    fn editor_pane(
        &mut self,
        can_write: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let find_bar = self.find_bar(can_write, window, cx);
        let Some(opened) = &self.opened else {
            return div().into_any_element();
        };
        match &opened.content {
            Content::Loading => div()
                .id("files-loading-file")
                .accessible(Role::Status, "files-loading-file", t(M::FilesLoadingFile))
                .size_full()
                .p(px(16.))
                .bg(colors::GRAY_850)
                .child(skeleton_code(16))
                .into_any_element(),
            Content::Error(error) => graphic(
                Icon::FilePen,
                t(M::FilesEditorTitle),
                Some(error.clone().into()),
            )
            .into_any_element(),
            Content::Loaded(_) => div()
                .size_full()
                .flex()
                .flex_col()
                .children(find_bar)
                .child(div().flex_1().min_h_0().child(self.editor.clone()))
                .into_any_element(),
        }
    }
}

/// The archive name the context menu's Zip makes (`generateZipFileName`).
fn zip_name(files: &[ClientFile]) -> String {
    match files {
        [one] => format!("{}.zip", one.name),
        _ => "Archive.zip".into(),
    }
}

/// A path as the core lists it: `./config` is `config`. (The Host joins names to the
/// top folder `.`, while the core's paths start without it.)
fn listed(path: &str, sep: char) -> &str {
    let mut path = path;
    while let Some(rest) = path.strip_prefix('.').and_then(|p| p.strip_prefix(sep)) {
        path = rest;
    }
    path
}

/// Whether `path` is `ancestor` or inside it, part by part: `config/create` holds
/// `config/create/a.toml`, but not `config/create-common.toml`.
fn is_within(path: &str, ancestor: &str, sep: char) -> bool {
    let (path, ancestor) = (
        listed(path, sep),
        listed(ancestor, sep).trim_end_matches(sep),
    );
    path == ancestor
        || path
            .strip_prefix(ancestor)
            .is_some_and(|rest| rest.starts_with(sep))
}

/// Where `path` is once `from` (it, or a folder it is in) moved to `to`.
fn moved_path(path: &str, from: &str, to: &str, sep: char) -> Option<String> {
    if !is_within(path, from, sep) {
        return None;
    }
    let (path, from) = (listed(path, sep), listed(from, sep).trim_end_matches(sep));
    Some(format!("{}{}", listed(to, sep), &path[from.len()..]))
}

/// Where each of several downloads into `dir` is saved: under its own name, or
/// `name (2).ext`, `name (3).ext`, … when a file there (`exists`) or an earlier one of
/// these downloads has it, so none replaces another or a file the user had.
fn download_paths(dir: &Path, names: &[String], exists: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    // names differing only in case are the same file on macOS and Windows
    let mut taken: Vec<String> = Vec::new();
    names
        .iter()
        .map(|name| {
            let (stem, extension) = {
                let path = Path::new(name);
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| name.clone());
                let extension = path
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()));
                (stem, extension.unwrap_or_default())
            };
            let out = (1..)
                .map(|n| match n {
                    1 => name.clone(),
                    n => format!("{stem} ({n}){extension}"),
                })
                .map(|candidate| dir.join(candidate))
                .find(|out| {
                    let key = out.to_string_lossy().to_lowercase();
                    !taken.contains(&key) && !exists(out)
                })
                .expect("an unused name");
            taken.push(out.to_string_lossy().to_lowercase());
            out
        })
        .collect()
}

/// Shows a toast through the Host, for work that outlives the page.
fn toast_later(state: &WeakEntity<HostState>, toast: Toast, cx: &mut AsyncApp) {
    state.update(cx, |state, cx| state.toast(toast, cx)).ok();
}

/// Has the instance's Files tab, whichever page shows it now, list its folder again.
fn files_changed(state: &WeakEntity<HostState>, uuid: &str, cx: &mut AsyncApp) {
    let uuid = uuid.to_string();
    state
        .update(cx, |_, cx| cx.emit(HostEvent::FilesChanged(uuid)))
        .ok();
}

/// Whether a folder's `files` have one named `name`. Case is ignored, as macOS's and
/// Windows' file systems do, where `Server.properties` is `server.properties`.
fn name_taken(files: &[ClientFile], name: &str) -> bool {
    let name = name.to_lowercase();
    files.iter().any(|file| file.name.to_lowercase() == name)
}

/// The name a download is saved under: a folder comes as a zip.
fn download_name(file: &ClientFile) -> String {
    if file.is_dir() {
        format!("{}.zip", file.name)
    } else {
        file.name.clone()
    }
}

/// When a file changed: "x ago" within a day, else its date (`formatTimeAgo`).
fn time_label(unix: u64) -> String {
    let now = chrono::Utc::now().timestamp();
    let age = now - unix as i64;
    if age < 60 {
        lodestone_ui::i18n::t(FormatMsg::JustNow).to_string()
    } else if age < 3600 {
        trn!(FormatPlural::MinutesAgo, age / 60)
    } else if age < 86_400 {
        trn!(FormatPlural::HoursAgo, age / 3600)
    } else {
        chrono::DateTime::from_timestamp(unix as i64, 0)
            .map(|time| {
                let local = time.with_timezone(&chrono::Local).date_naive();
                let this_year = chrono::Local::now().date_naive().format("%Y").to_string()
                    == local.format("%Y").to_string();
                format::short_date(local, !this_year)
            })
            .unwrap_or_default()
    }
}

/// The dashboard's icon for a file (`FileList.tsx`'s `iconMap`).
fn file_icon(file: &ClientFile) -> AnyElement {
    let path = match file.file_type {
        FileType::Directory => "folder-blue",
        FileType::Unknown => {
            return div()
                .flex_none()
                .child(Icon::ClipboardQuestion.render(px(16.), colors::YELLOW))
                .into_any_element();
        }
        FileType::File
            if file.name == ".lodestone_config" || file.file_stem == ".lodestone_config" =>
        {
            "lodestone"
        }
        FileType::File => match file
            .extension
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("txt" | "md" | "properties" | "cfg" | "conf" | "config" | "ini") => "document",
            Some("yml" | "yaml") => "yaml",
            Some("exe") => "exe",
            Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg") => "image",
            Some("json" | "mcmeta") => "json",
            Some("zip" | "gz" | "tar") => "zip",
            Some("jar") => "jar",
            Some("java") => "java",
            Some("class") => "javaclass",
            Some("ts") => "typescript",
            Some("js") => "javascript",
            Some("readme") => "readme",
            Some("bin" | "dll" | "dylib" | "so" | "o") => "binary",
            Some("dat" | "dat_old" | "obj" | "nbt" | "mca") => "3d",
            Some("lock") => "lock",
            Some("log") => "log",
            Some("ps1") => "pwsh",
            Some("sh") => "console",
            Some("mcfunction") => "minecraft",
            _ => "file",
        },
    };
    img(SharedString::from(format!("images/file-icons/{path}.svg")))
        .flex_none()
        .size(px(16.))
        .into_any_element()
}

type MenuHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

fn menu_item(
    id: &'static str,
    label: impl Into<SharedString>,
    icon: Option<Icon>,
    disabled: bool,
    danger: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let label = label.into();
    let color = if disabled {
        colors::GRAY_500
    } else if danger {
        colors::RED_200
    } else {
        colors::GRAY_300
    };
    let handler: MenuHandler = Box::new(on_click);
    div()
        .id(id)
        .accessible(Role::MenuItem, id, label.clone())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .px(px(10.))
        .py(px(6.))
        .rounded(radius::DEFAULT)
        .whitespace_nowrap()
        .text_size(text::SMALL)
        .font_weight(weights::MEDIUM.face())
        .text_color(color)
        .when(!disabled, |this| {
            this.cursor_pointer()
                .hover(|style| style.bg(colors::GRAY_700))
                .on_press(move |event, window, cx| handler(event, window, cx))
        })
        .when_some(icon, |this, icon| this.child(icon.render(px(13.), color)))
        .child(label)
        .into_any_element()
}

/// A menu at `at`, its groups divided by lines; kept inside the window.
fn menu_panel(
    at: Point<Pixels>,
    groups: Vec<Vec<AnyElement>>,
    cx: &mut Context<FilesPage>,
) -> impl IntoElement + use<> {
    deferred(
        anchored()
            .position(at)
            .snap_to_window_with_margin(px(8.))
            .child(
                div()
                    .id("files-menu")
                    .key_context("FilesMenu")
                    .accessible(Role::Menu, "files-menu", t(CommonMsg::Menu))
                    .occlude()
                    .mt(px(6.))
                    .min_w(px(176.))
                    .max_w(px(320.))
                    .flex()
                    .flex_col()
                    .rounded(radius::DEFAULT)
                    .border_1()
                    .border_color(colors::GRAY_FADED.opacity(30))
                    .bg(colors::GRAY_900)
                    .shadow_md()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.menu = None;
                        cx.notify();
                    }))
                    .children(groups.into_iter().enumerate().map(|(i, items)| {
                        div()
                            .flex()
                            .flex_col()
                            .p(px(4.))
                            .when(i > 0, |this| {
                                this.border_t_1()
                                    .border_color(colors::GRAY_FADED.opacity(30))
                            })
                            .children(items)
                    })),
            ),
    )
    .with_priority(1)
}

impl Render for FilesPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_read = self.can_read(cx);
        let can_write = self.can_write(cx);
        let editor_pane = self.editor_pane(can_write, window, cx);
        let showing_editor = matches!(
            self.opened.as_ref().map(|o| &o.content),
            Some(Content::Loaded(_))
        );
        let menu = self.menu.as_ref().map(|menu| match menu {
            Menu::Toolbar(at) => (*at, None, true),
            Menu::Context { at, file } => (*at, file.clone(), false),
        });
        let menu = menu.map(|(at, file, toolbar)| {
            let groups = if toolbar {
                self.toolbar_menu(cx)
            } else {
                self.context_menu(file, cx)
            };
            menu_panel(at, groups, cx)
        });

        // the toolbar
        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(
                div()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.menu = match this.menu {
                                Some(Menu::Toolbar(_)) => None,
                                _ => Some(Menu::Toolbar(event.position)),
                            };
                            cx.notify();
                        }),
                    )
                    .child(
                        Button::new("files-add-remove", t(M::FilesAddRemove))
                            .icon_right(Icon::CaretDown)
                            .disabled(!can_read),
                    ),
            )
            .child(self.breadcrumb(cx))
            .when(!can_write && self.opened.is_some(), |this| {
                this.child(
                    div()
                        .flex_none()
                        .text_size(text::SMALL)
                        .text_color(colors::GRAY_400)
                        .child(t(M::FilesReadOnly)),
                )
            })
            .when(!self.clipboard.is_empty(), |this| {
                this.child(
                    Button::new(
                        "files-paste-toolbar",
                        trn!(P::FilesPasteCount, self.clipboard.len()),
                    )
                    .icon(Icon::Paste)
                    .disabled(!can_write)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let dir = this.path.clone();
                        this.paste(dir, cx);
                    })),
                )
            })
            .when(showing_editor, |this| {
                this.child(
                    Button::new("files-save", t(CommonMsg::Save))
                        .icon(Icon::FloppyDisk)
                        .loading(self.saving)
                        .disabled(!self.dirty || !can_write || self.saving)
                        .on_click(cx.listener(|this, _, _, cx| this.save(None, cx))),
                )
                .child(
                    Button::new("files-discard", t(M::FilesDiscardChanges))
                        .icon(Icon::ArrowsRotate)
                        .disabled(!self.dirty)
                        .on_click(cx.listener(|this, _, _, cx| this.discard(cx))),
                )
            })
            .child(
                Button::new("files-upload", t(M::FilesUpload))
                    .icon(Icon::Upload)
                    .disabled(!can_write)
                    .on_click(cx.listener(|this, _, _, cx| this.choose_uploads(cx))),
            );

        let panel: AnyElement = if !can_read {
            div()
                .flex_1()
                .min_h_0()
                .rounded(radius::LG)
                .border_1()
                .border_color(colors::GRAY_FADED.opacity(30))
                .child(graphic(Icon::Folder, t(M::FilesNoReadPermission), None))
                .into_any_element()
        } else {
            let editing = self.opened.is_some();
            let list = div()
                .id("files-list")
                .accessible(Role::List, "files-list", t(M::FilesList))
                .h_full()
                .when(editing, |this| this.w(px(self.list_width)).flex_none())
                .when(!editing, |this| this.flex_1())
                .min_w(px(LIST_MIN))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded_l(radius::LG)
                .when(!editing, |this| this.rounded_r(radius::LG))
                .on_click(cx.listener(|this, _, _, cx| {
                    // a click on the empty space: closes the file, and unticks
                    if this.guard(Pending::Close, cx) {
                        return;
                    }
                    this.close_file(cx);
                    this.ticked.clear();
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                        this.menu = Some(Menu::Context {
                            at: event.position,
                            file: None,
                        });
                        cx.notify();
                    }),
                )
                .child(
                    uniform_list(
                        "files-rows",
                        self.row_count(),
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            range.map(|ix| this.row(ix, cx)).collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(&self.list_scroll)
                    .flex_1(),
                )
                .child(div().h(px(48.)).flex_none());
            div()
                .id("files-panel")
                .flex_1()
                .min_h_0()
                .flex()
                .flex_row()
                .rounded(radius::LG)
                .border_1()
                .border_color(colors::GRAY_FADED.opacity(30))
                .bg(colors::GRAY_800)
                .child(list)
                .when(editing, |this| {
                    this.child(
                        div()
                            .id("files-divider")
                            .accessible(Role::Splitter, "files-divider", t(M::FilesResize))
                            .w(px(5.))
                            .h_full()
                            .flex_none()
                            .cursor_col_resize()
                            .border_l_1()
                            .border_color(colors::GRAY_FADED.opacity(30))
                            .hover(|style| style.bg(colors::GRAY_700))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                    this.resizing = Some((event.position.x, this.list_width));
                                    cx.stop_propagation();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_hidden()
                            .rounded_r(radius::LG)
                            .child(editor_pane),
                    )
                })
                .into_any_element()
        };

        let status = {
            let mut parts = Vec::new();
            if !self.ticked.is_empty() {
                parts.push(trn!(P::FilesSelected, self.ticked.len()));
            }
            if !self.clipboard.is_empty() {
                parts.push(trn!(P::FilesInClipboard, self.clipboard.len()));
            }
            let wrap = self.editor.read(cx).soft_wrap();
            let editor = self.editor.clone();
            div()
                .h(px(20.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(16.))
                .px(px(16.))
                .text_size(text::MEDIUM)
                .font_weight(weights::MEDIUM.face())
                .text_color(lodestone_ui::theme::ink::muted())
                .children(parts)
                .when(showing_editor, |this| {
                    // long lines wrap, or scroll sideways
                    this.child(div().flex_1()).child(
                        Checkbox::new("files-wrap", wrap, t(M::FilesWrapLines)).on_change(
                            move |wrap, _, cx| {
                                let wrap = *wrap;
                                editor.update(cx, |editor, cx| editor.set_soft_wrap(wrap, cx));
                                crate::prefs::Prefs::update(cx, |prefs| prefs.editor_wrap = wrap);
                            },
                        ),
                    )
                })
        };

        if std::mem::take(&mut self.focus_name) {
            let handle = gpui::Focusable::focus_handle(self.name_input.read(cx), cx);
            window.focus(&handle, cx);
        }
        let dialogs = self
            .dialog
            .render(window, |dialog| Some(self.render_dialog(dialog, cx)));

        div()
            .id("files-page")
            .key_context("FilesPage")
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(|this, _: &FindInFile, _, cx| this.open_find(cx)))
            .on_action(cx.listener(Self::close_menu))
            .relative()
            .size_full()
            .min_h(px(360.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if let Some((start, width)) = this.resizing {
                    if event.pressed_button != Some(MouseButton::Left) {
                        this.resizing = None;
                        return;
                    }
                    let width = (width + f32::from(event.position.x - start)).max(LIST_MIN);
                    this.list_width = width;
                    cx.default_global::<Remembered>().list_width = Some(width);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    // the width dragged to is kept for the next start
                    if this.resizing.take().is_some() {
                        let width = this.list_width;
                        crate::prefs::Prefs::update(cx, |prefs| {
                            prefs.file_list_width = Some(width)
                        });
                    }
                }),
            )
            // while files are dragged over the tab, its edge and a tint say they can be
            // dropped (as the launcher's Content tab does)
            .rounded(radius::LG)
            .border_2()
            .border_color(colors::TRANSPARENT)
            .when(can_write, |this| {
                this.drag_over::<ExternalPaths>(|style, _, _, _| {
                    style
                        .border_color(lodestone_ui::theme::accent::light().opacity(60))
                        .bg(lodestone_ui::theme::accent::base().opacity(8))
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                    this.upload(paths.paths().to_vec(), cx)
                }))
            })
            .child(toolbar)
            .child(panel)
            .child(status)
            .children(menu)
            .children(window_layer(window, dialogs))
            .font_family(fonts::body())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, file_type: FileType) -> ClientFile {
        ClientFile {
            name: name.into(),
            file_stem: name.split('.').next().unwrap_or(name).into(),
            extension: name.rsplit_once('.').map(|(_, e)| e.into()),
            path: name.into(),
            size: Some(10),
            creation_time: None,
            modification_time: None,
            file_type,
        }
    }

    #[test]
    fn zip_and_download_names() {
        let a = file("world", FileType::Directory);
        let b = file("server.properties", FileType::File);
        assert_eq!(zip_name(std::slice::from_ref(&a)), "world.zip");
        assert_eq!(zip_name(&[a.clone(), b.clone()]), "Archive.zip");
        assert_eq!(download_name(&a), "world.zip");
        assert_eq!(download_name(&b), "server.properties");
    }

    #[test]
    fn a_new_name_clashes_with_any_entry_there_whatever_its_case() {
        let files = [
            file("server.properties", FileType::File),
            file("world", FileType::Directory),
        ];
        assert!(name_taken(&files, "server.properties"));
        assert!(name_taken(&files, "Server.Properties"));
        // a new file can't take a folder's name either
        assert!(name_taken(&files, "world"));
        assert!(!name_taken(&files, "ops.json"));
        assert!(!name_taken(&files, "server.properties.bak"));
    }

    #[test]
    fn a_folder_holds_only_paths_under_it_part_by_part() {
        let sep = '/';
        assert!(is_within("config/create/a.toml", "config/create", sep));
        assert!(is_within("config/create", "config/create", sep));
        assert!(is_within("config/create/a.toml", "config/create/", sep));
        // names that only start the same aren't in it
        assert!(!is_within(
            "config/create-common.toml",
            "config/create",
            sep
        ));
        assert!(!is_within("world_nether/paper-world.yml", "world", sep));
        assert!(!is_within("server.properties", "server", sep));
        // the Host's paths from the top folder are the core's
        assert!(is_within("world/level.dat", "./world", sep));
        assert!(is_within("./world/level.dat", "world", sep));
        // Windows cores use backslashes
        assert!(is_within(r"config\create\a.toml", r"config\create", '\\'));
        assert!(!is_within(
            r"config\create-common.toml",
            r"config\create",
            '\\'
        ));
    }

    #[test]
    fn a_moved_file_is_found_under_its_new_folder() {
        let sep = '/';
        assert_eq!(
            moved_path("config/create/a.toml", "config/create", "./old/create", sep).as_deref(),
            Some("old/create/a.toml")
        );
        assert_eq!(
            moved_path(
                "server.properties",
                "server.properties",
                "./a.properties",
                sep
            )
            .as_deref(),
            Some("a.properties")
        );
        assert_eq!(
            moved_path("config/create-common.toml", "config/create", "x", sep),
            None
        );
    }

    #[test]
    fn several_downloads_never_replace_a_file_or_each_other() {
        let dir = Path::new("downloads");
        let had = [dir.join("world.zip"), dir.join("world (2).zip")];
        let names = [
            "world.zip".to_string(),
            "world.zip".to_string(),
            "server.properties".to_string(),
            "Server.properties".to_string(),
            "eula".to_string(),
        ];
        let outs = download_paths(dir, &names, |path| had.iter().any(|h| h == path));
        assert_eq!(
            outs,
            [
                dir.join("world (3).zip"),
                dir.join("world (4).zip"),
                dir.join("server.properties"),
                // the same file as the one before on macOS and Windows
                dir.join("Server (2).properties"),
                dir.join("eula"),
            ]
        );
    }

    #[test]
    fn recent_times_say_how_long_ago() {
        let now = chrono::Utc::now().timestamp() as u64;
        assert_eq!(time_label(now), "Just now");
        assert_eq!(time_label(now - 120), "2 minutes ago");
        assert_eq!(time_label(now - 3 * 3600), "3 hours ago");
        assert!(!time_label(now - 40 * 86_400).contains("ago"));
    }
}
