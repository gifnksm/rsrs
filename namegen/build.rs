use io::{BufRead as _, BufReader, BufWriter, Write as _};
use std::{
    env,
    fs::File,
    io,
    path::{Path, PathBuf},
};

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    gen(&out_dir, "adjectives")?;
    gen(&out_dir, "animals")?;

    Ok(())
}

fn gen(out_dir: impl AsRef<Path>, name: &str) -> io::Result<()> {
    let out_dir = out_dir.as_ref();

    let mut src_path = PathBuf::from("data");
    src_path.push(name);
    src_path.set_extension("txt");
    println!("cargo:rerun-if-changed={}", src_path.display());

    let mut dest_path = out_dir.to_path_buf();
    dest_path.push(name);
    dest_path.set_extension("rs");

    let src = BufReader::new(File::open(src_path).unwrap());
    let mut dest = BufWriter::new(File::create(dest_path).unwrap());

    writeln!(&mut dest, "pub const {}: &[&str] = &[", name.to_uppercase())?;
    for line in src.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        writeln!(&mut dest, "    {:?},", line)?;
    }
    writeln!(&mut dest, "];")?;

    Ok(())
}
