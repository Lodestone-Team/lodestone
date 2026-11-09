//! The permission editor: a switch for each core-wide permission and a grid of
//! instances for each per-instance one, after the dashboard's user permissions
//! (`UserSettings.tsx`) and an API key's custom scope (`ApiKeyScopeEditor.tsx`).

use std::rc::Rc;

use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window, div};
use launcher_core::cores::admin::{InstanceInfo, UserPermission, users::Permission};
use lodestone_ui::{
    components::{Checkbox, Toggle, settings::SettingRow},
    theme::{colors, ink, radius, text},
};

use crate::i18n::{CoreMsg, t};

/// A permission's name, what it lets a user do, and what it lets a key do.
pub fn messages(permission: Permission) -> (CoreMsg, CoreMsg, CoreMsg) {
    use CoreMsg::*;
    match permission {
        Permission::CreateInstance => (
            PermCreateInstance,
            PermCreateInstanceUser,
            PermCreateInstanceKey,
        ),
        Permission::DeleteInstance => (
            PermDeleteInstance,
            PermDeleteInstanceUser,
            PermDeleteInstanceKey,
        ),
        Permission::ReadGlobalFile => (
            PermReadGlobalFile,
            PermReadGlobalFileUser,
            PermReadGlobalFileKey,
        ),
        Permission::ViewInstance => (PermViewInstance, PermViewInstanceUser, PermViewInstanceKey),
        Permission::StartInstance => (
            PermStartInstance,
            PermStartInstanceUser,
            PermStartInstanceKey,
        ),
        Permission::StopInstance => (PermStopInstance, PermStopInstanceUser, PermStopInstanceKey),
        Permission::Console => (PermConsole, PermConsoleUser, PermConsoleKey),
        Permission::InstanceSettings => (
            PermInstanceSettings,
            PermInstanceSettingsUser,
            PermInstanceSettingsKey,
        ),
        Permission::ReadResource => (PermReadResource, PermReadResourceUser, PermReadResourceKey),
        Permission::ReadFiles => (PermReadFiles, PermReadFilesUser, PermReadFilesKey),
        Permission::WriteResource => (
            PermWriteResource,
            PermWriteResourceUser,
            PermWriteResourceKey,
        ),
        Permission::WriteFiles => (PermWriteFiles, PermWriteFilesUser, PermWriteFilesKey),
        Permission::WriteGlobalFile => (
            PermWriteGlobalFile,
            PermWriteGlobalFileUser,
            PermWriteGlobalFileKey,
        ),
        Permission::ManagePermission => (
            PermManagePermission,
            PermManagePermissionUser,
            PermManagePermissionKey,
        ),
    }
}

/// A change: the permission, the instance (for a per-instance one) and the new value.
pub type OnChange = Rc<dyn Fn(Permission, Option<String>, bool, &mut Window, &mut App)>;

/// Whether a permission (on an instance, or `None` for the whole row) can be changed.
pub type Allowed<'a> = &'a dyn Fn(Permission, Option<&str>) -> bool;

/// One row per permission in `list`, from `value`. `for_key` words the hints for an
/// API key; `allowed` disables what can't be changed; `extra_hint` is added after the
/// hint of a row that can't be changed at all.
#[allow(clippy::too_many_arguments)]
pub fn rows(
    id: &str,
    list: &[Permission],
    value: &UserPermission,
    instances: &[InstanceInfo],
    for_key: bool,
    allowed: Allowed,
    extra_hint: Option<&str>,
    on_change: OnChange,
) -> Vec<AnyElement> {
    list.iter()
        .map(|&permission| {
            let (title, user, key) = messages(permission);
            let row_allowed = if permission.per_instance() {
                instances.iter().any(|i| allowed(permission, Some(&i.uuid)))
            } else {
                allowed(permission, None)
            };
            let mut hint = t(if for_key { key } else { user }).to_string();
            if !row_allowed && let Some(extra) = extra_hint {
                hint = format!("{hint} {extra}");
            }
            let row_id = SharedString::from(format!("{id}-{permission:?}"));
            let row = SettingRow::new(t(title)).key(row_id.clone()).hint(hint);
            match permission.list(value) {
                Some(granted) => {
                    let grid: AnyElement = if instances.is_empty() {
                        div()
                            .text_size(text::MEDIUM)
                            .text_color(ink::muted())
                            .child(t(CoreMsg::NoServersYet))
                            .into_any_element()
                    } else {
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap_x_4()
                            .gap_y_2()
                            .px_4()
                            .py_3()
                            .rounded(radius::LG)
                            .border_1()
                            .border_color(ink::border())
                            .bg(colors::GRAY_900)
                            .children(instances.iter().enumerate().map(|(index, instance)| {
                                let checked = granted.contains(&instance.uuid);
                                let uuid = instance.uuid.clone();
                                let on_change = on_change.clone();
                                Checkbox::new(
                                    (row_id.clone(), index),
                                    instance.name.clone(),
                                    checked,
                                )
                                .disabled(!allowed(permission, Some(&instance.uuid)))
                                .on_change(
                                    move |value, window, cx| {
                                        on_change(
                                            permission,
                                            Some(uuid.clone()),
                                            *value,
                                            window,
                                            cx,
                                        )
                                    },
                                )
                            }))
                            .into_any_element()
                    };
                    row.below(grid).into_any_element()
                }
                None => {
                    let on_change = on_change.clone();
                    row.child(
                        Toggle::new(row_id.clone(), permission.flag(value).unwrap_or(false))
                            .label(t(title))
                            .disabled(!row_allowed)
                            .on_change(move |value, window, cx| {
                                on_change(permission, None, *value, window, cx)
                            }),
                    )
                    .into_any_element()
                }
            }
        })
        .collect()
}
