//! The system's preferred languages, most preferred first, for the Language
//! setting's "System".

/// On macOS, `+[NSLocale preferredLanguages]`: the list in System Settings > General >
/// Language & Region, as tags such as `fr-FR`, `zh-Hans-CN` or `en-GB`.
///
/// GPUI does not expose it and the launcher has no Objective-C crate, so it is read
/// through the Objective-C runtime that AppKit already loads, as
/// `components::dialog` reads the reduced-motion setting.
#[cfg(target_os = "macos")]
pub fn preferred_languages() -> Vec<String> {
    use std::ffi::{CStr, c_char, c_void};

    type Id = *mut c_void;
    type Sel = *mut c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
    }

    let mut languages = Vec::new();
    // SAFETY: `objc_msgSend` is called through function types that match each method:
    // `+[NSLocale preferredLanguages]` takes no arguments and returns an `NSArray`,
    // `-[NSArray count]` returns an `NSUInteger` (`usize`), `-[NSArray
    // objectAtIndex:]` takes an `NSUInteger` and returns an object, here an
    // `NSString`, and `-[NSString UTF8String]` returns a C string that lives as long as
    // the string. All exist since macOS 10.5. A missing class and null results are
    // checked before they are used, and the C string is copied before the next
    // message is sent.
    unsafe {
        let class = objc_getClass(c"NSLocale".as_ptr());
        if class.is_null() {
            return languages;
        }
        let send_id = std::mem::transmute::<
            unsafe extern "C" fn(),
            unsafe extern "C" fn(Id, Sel) -> Id,
        >(objc_msgSend);
        let send_count = std::mem::transmute::<
            unsafe extern "C" fn(),
            unsafe extern "C" fn(Id, Sel) -> usize,
        >(objc_msgSend);
        let send_index = std::mem::transmute::<
            unsafe extern "C" fn(),
            unsafe extern "C" fn(Id, Sel, usize) -> Id,
        >(objc_msgSend);
        let send_utf8 = std::mem::transmute::<
            unsafe extern "C" fn(),
            unsafe extern "C" fn(Id, Sel) -> *const c_char,
        >(objc_msgSend);
        let list = send_id(class, sel_registerName(c"preferredLanguages".as_ptr()));
        if list.is_null() {
            return languages;
        }
        let count = send_count(list, sel_registerName(c"count".as_ptr()));
        let object_at = sel_registerName(c"objectAtIndex:".as_ptr());
        let utf8 = sel_registerName(c"UTF8String".as_ptr());
        // A handful of languages; the cap guards against a nonsense count.
        for index in 0..count.min(32) {
            let string = send_index(list, object_at, index);
            if string.is_null() {
                continue;
            }
            let chars = send_utf8(string, utf8);
            if !chars.is_null() {
                languages.push(CStr::from_ptr(chars).to_string_lossy().into_owned());
            }
        }
    }
    languages
}

/// Elsewhere, the POSIX locale variables, in the order gettext reads them:
/// `LANGUAGE` (a list, `fr:en`), then `LC_ALL`, `LC_MESSAGES` and `LANG`
/// (`fr_FR.UTF-8`). On Windows, the user's preferred UI languages follow these
/// variables, which are usually unset.
#[cfg(not(target_os = "macos"))]
pub fn preferred_languages() -> Vec<String> {
    let mut languages = Vec::new();
    if let Ok(list) = std::env::var("LANGUAGE") {
        languages.extend(
            list.split(':')
                .filter(|tag| !tag.is_empty())
                .map(str::to_string),
        );
    }
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(name)
            // "C" and "POSIX" mean no language.
            && !value.is_empty()
            && value != "C"
            && value != "POSIX"
        {
            languages.push(value);
            break;
        }
    }
    #[cfg(windows)]
    languages.extend(windows_languages());
    languages
}

#[cfg(windows)]
fn windows_languages() -> Vec<String> {
    use windows::{
        Win32::{
            Globalization::{
                GetUserDefaultLocaleName, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME,
            },
            System::SystemServices::LOCALE_NAME_MAX_LENGTH,
        },
        core::PWSTR,
    };

    let mut count = 0;
    let mut length = 0;
    // SAFETY: The first call obtains the required length; the second writes to a
    // buffer of that length. Both output pointers and the fallback buffer are valid.
    unsafe {
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut length).is_ok()
            && length > 0
        {
            let mut buffer = vec![0; length as usize];
            if GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut count,
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut length,
            )
            .is_ok()
            {
                let languages = decode_windows_languages(&buffer);
                if !languages.is_empty() {
                    return languages;
                }
            }
        }
        let mut buffer = [0; LOCALE_NAME_MAX_LENGTH as usize];
        if GetUserDefaultLocaleName(&mut buffer) > 0 {
            return decode_windows_languages(&buffer);
        }
    }
    Vec::new()
}

#[cfg(windows)]
fn decode_windows_languages(buffer: &[u16]) -> Vec<String> {
    buffer
        .split(|&unit| unit == 0)
        .take_while(|tag| !tag.is_empty())
        .filter_map(|tag| String::from_utf16(tag).ok())
        .collect()
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn decodes_windows_language_list() {
        let tags: Vec<u16> = "de-DE\0zh-TW\0fr-FR\0\0".encode_utf16().collect();
        let languages = super::decode_windows_languages(&tags);
        assert_eq!(languages, ["de-DE", "zh-TW", "fr-FR"]);
        assert_eq!(
            languages
                .iter()
                .find_map(|tag| crate::i18n::Locale::from_tag(tag)),
            Some(crate::i18n::Locale::ZhHant)
        );
        assert!(super::decode_windows_languages(&[0, 0]).is_empty());
    }
}
