use std::{
    fs::File,
    io::{SeekFrom, Write},
};

use tempfile;

/// Generate an xkb_keymap content.
///
/// Specifically, we're trying to generate a keymap where each keycode
/// matches its Unicode codepoint. :)
fn make_keymap() -> String {
    let mut keycodes = String::new();
    let mut symbols = String::new();

    for i in 0..=9 {
        let code = i + '0' as u32;
        keycodes += &format!(
            r#"
    <N{i}> = {code}
"#
        );
        symbols += &format!(
            r#"
    key <N{i}> {{ [ {i} ] }};
"#
        );
    }
    for c in 0..26 {
        let lower = c + 'a' as u32;
        let upper = c + 'A' as u32;
        let lower_c = unsafe { char::from_u32_unchecked(lower) };
        let upper_c = unsafe { char::from_u32_unchecked(upper) };
        keycodes += &format!(
            r#"
    <LO{c}> = {lower};
    <UP{c}> = {upper};
"#
        );
        symbols += &format!(
            r#"
    key <LO{c}> {{ type= "ALPHABETIC", symbols[Group1]= [ {lower_c} ] }};
    key <UP{c}> {{ type= "ALPHABETIC", symbols[Group1]= [ {upper_c} ] }};
"#
        );
    }

    format!(
        r#"
xkb_keymap {{
xkb_keycodes "alphabetic" {{
    minimum = 8;
    maximum = 255;
    {keycodes}
}};
xkb_types "alphabetic" {{ }};
xkb_compatibility "alphabetic" {{ }};
xkb_symbols "alphabetic" {{
    name[group1]="English (US)";
    {symbols}
}};
}};
"#
    )
}

pub fn get_temp_keymap() -> Result<File, String> {
    let mut f = tempfile::tempfile().map_err(|e| format!("error creating keymap tempfile: {e}"))?;
    let _ = f
        .write_all(make_keymap().as_bytes())
        .map_err(|e| format!("error writing to keymap tempfile: {e}"));
    let _ = f
        .flush()
        .map_err(|e| format!("error finishing write of keymap tempfile: {e}"));
    use std::io::Seek;
    f.seek(SeekFrom::Start(0))
        .map_err(|e| format!("error rewinding keymap tempfile: {e}"))?;
    Ok(f)
}
