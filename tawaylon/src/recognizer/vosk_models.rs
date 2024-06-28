// Embed model into the binary;
// extract it to a temporary directory,
// once.
//
use std::{path::PathBuf, sync::OnceLock};
use tempfile::{tempdir, TempDir};

const MODEL_ZIP: &[u8] = include_bytes!("../../../models/vosk-small.zip");
static MODEL_DIR: OnceLock<Result<(TempDir, PathBuf), String>> = OnceLock::new();

fn init_model() -> Result<(TempDir, PathBuf), String> {
    let dir = tempdir().map_err(|err| format!("error preparing for decompression: {err}"))?;
    let reader = std::io::Cursor::new(MODEL_ZIP);
    let mut zipfile = zip::read::ZipArchive::new(reader)
        .map_err(|err| format!("error decompressing models: {err}"))?;
    zipfile
        .extract(dir.path())
        .map_err(|err| format!("error decompressing models: {err}"))?;
    let path = dir.path();
    let inner = std::fs::read_dir(path)
        .map_err(|err| format!("could not read model directory: {err}"))?
        .next()
        .ok_or_else(|| "could not find inner model directory".to_owned())?
        .map_err(|err| format!("could not find inner model directory: {err}"))?
        .path()
        .to_owned();
    Ok((dir, inner))
}

pub fn get() -> Result<PathBuf, String> {
    match MODEL_DIR.get_or_init(init_model) {
        Ok((_, inner)) => Ok(inner.clone()),
        Err(e) => Err(e.clone()),
    }
}
