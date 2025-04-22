use std::env;
use std::fs;
use std::io;
use std::io::stdout;
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use::std::path::Path;
use std::process::Command;
use anyhow::Context;
use anyhow::Error;
use chrono;
use walkdir::WalkDir;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;
use std::fs::File;
fn main() {
  let args: Vec<String> = env::args().collect();

  parse_args(&args);

  println!("::endgroup::");
}

struct TranslateHelperConfig {
  book_lang: String,
  dest_dir: String
}

fn build_translation(locale: String, dest_dir: String) -> Result<(), Error>{
  if locale == "en" {
    println!("::group::Building English course");
  } else {
    let locale_ref = &locale;
    let file = fs::File::open(format!("po/{locale_ref}.po"))?;
    let reader = io::BufReader::new(file);

    let mut pot_creation_date: Option<String> = None;

    for line in reader.lines() {
      let line = line.expect("Error reading line");
      if line.contains("POT-Creation-Date") {
        println!("{line}");
        let line_parts: Vec<&str> = line.split(":").collect();
        pot_creation_date = Some(line_parts[1].trim_start().to_string());
        println!("{:?}", pot_creation_date);
        break;
      }
    }

    if pot_creation_date.is_none() {
      let now = chrono::Local::now();
      pot_creation_date = Some(now.format("%Y-%m-%dT%H:%M:%S").to_string());
      // println!("Created date from now {:?}", pot_creation_date);
    }

    println!("::group::Building {locale_ref} translation as of {:?}", pot_creation_date.clone().unwrap());

    // Back-date the source to POT-Creation-Date. The content lives in two directories:
    fs::remove_dir_all("src");
    fs::remove_dir_all("third_party");
    // Command::new("git").arg("rev-list").arg("-n").arg("--")
    let output = Command::new("git").args(["rev-list", "-n", "1", "--before", &pot_creation_date.unwrap(), "@"]).output();
    let result_str: String;
    match output {
        Ok(result) => {
          result_str = String::from_utf8(result.stdout).unwrap().replace("\n", "");
          println!("result string: {:?}", result_str.clone());
        },
        Err(err) => {
          return Err(err.into());
        }
    }
    // io::stdout().write_all(&output.stdout).unwrap();
    let output2 = Command::new("git").args(["restore","--source", &result_str, "src/", "third_party/"]).output();

    env::set_var("MDBOOK_BOOK__LANGUAGE", locale_ref);
    env::set_var("MDBOOK_OUTPUT__HTML__SITE_URL", format!("/comprehensive-rust/{locale_ref}/"));
    env::set_var("MDBOOK_OUTPUT__HTML__REDIRECT", "{}");

    match output2 {
      Ok(result) => {
        println!("result string: {}", result.status);
        println!("result string: {:#?}", result.stdout);
        println!("result stderr string: {:#?}", String::from_utf8(result.stderr).unwrap());
      },
      Err(err) => {
        println!("inside err case");
        return Err(err.into());
      }
    }
  }

  // Enable mdbook-pandoc to build PDF version of the course
  env::set_var("MDBOOK_OUTPUT__PANDOC__DISABLED", "false");

  let dest_arg = format!("-d{dest_dir}");
  let output3 = Command::new("mdbook").arg("build").arg(dest_arg).output();
  match output3 {
    Ok(output) => {
      // println!("result string: {}", result.status);
      // println!("result string: {:?}", result.stdout);
      // println!("result stderr string: {:?}", String::from_utf8(result.stderr).unwrap());

      // println!("Exit status: {}", output.status);
      let stdout_str = String::from_utf8_lossy(&output.stdout);
      println!("\nStdout:");
      if stdout_str.is_empty() {
          println!("(empty)");
      } else {
          println!("{}", stdout_str);
      }

      // Capture stderr as well (often useful for debugging failures)
      let stderr_str = String::from_utf8_lossy(&output.stderr);
      if !stderr_str.is_empty() {
          eprintln!("\nStderr:"); // Use eprintln for errors
          eprintln!("{}", stderr_str);
      }

      if !output.status.success() {
          eprintln!("Command has non-zero exist status: {}", output.status);
          // return Err();
      }
    },
    Err(err) => {
      println!("inside err case");
      return Err(err.into());
    }
  }

  // Disable the redbox button in built versions of the course
  fs::write(format!("{}/html/theme/redbox.js", dest_dir), "// Disabled in published builds, see build.sh")?;

  let pdf_from_dir = format!("{}/pandoc/pdf/comprehensive-rust.pdf", dest_dir);
  let pdf_dest_dir = format!("{}/html/comprehensive-rust.pdf", dest_dir);

  let pdf_from_path = Path::new(&pdf_from_dir);
  let pdf_dest_path = Path::new(&pdf_dest_dir);

  fs::copy(pdf_from_path, pdf_dest_path)?;
  fs::remove_file(pdf_from_path)?;
  println!("Copied comprehensive-rust.pdf to {pdf_dest_dir}");

  let src_dir = format!("{dest_dir}/exerciser/comprehensive-rust-exercises");
  let src_path = Path::new(&src_dir);
  if !src_path.is_dir() {
    return Err(ZipError::FileNotFound.into());
  }

  let zip_file_str = format!("{dest_dir}/html/comprehensive-rust-exercises.zip");
  let zip_file = File::create(Path::new(&zip_file_str))?;

  let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

  let mut zip = zip::ZipWriter::new(zip_file);
  let mut buffer = Vec::new();

  for entry in WalkDir::new(src_path).into_iter().filter_map(|e| e.ok()) {
    let path = entry.path();
    let name = path.strip_prefix(src_path)?;
    let name_as_str = name.to_str().map(str::to_owned).with_context(|| format!("{name:?} is a non utf-8 path"))?;

    if path.is_file() {
      zip.start_file(name_as_str, options)?;
      let mut f = File::open(path)?;

      f.read_to_end(&mut buffer)?;
      zip.write_all(&buffer)?;
      buffer.clear();
    } else if !name.as_os_str().is_empty() {
      zip.add_directory(name_as_str, options)?;
    }
  }
  zip.finish()?;

  Ok(())
}


// should run msmerge
fn update_translation(locale: String, dest_dir: String) {
  print!("Updating translation for locale {locale} and destination directory {dest_dir}")
}

fn translate_all() {
  // let languages = env::var("LANGUAGES");
  if let Ok(languages) = env::var("LANGUAGES") {
    println!("Translation all languages in $LANGUAGES: {languages}");
    languages.split(" ").for_each(| lang | -> () {
      println!("translation language {lang}");
    });
  } else {
    panic!("Error reading environment variable LANGUAGES. Has it been set correctly?")
  }
}

fn parse_args(args: &[String]) -> () {
  let cmd_name = &args[0];

  if args.len() == 1 {
      panic!("No arguments provided. Usage {cmd_name} <action>")
  }
  let translate_action = args[1].clone();

  match translate_action.as_str() {
    "build" | "update" => {
      if args.len() != 4 {
        panic!("Usage: {cmd_name} <book-lang> <dest-dir>");
      }
      let config = TranslateHelperConfig { book_lang: args[2].clone(), dest_dir: args[3].clone() };

      if translate_action.as_str() == "build" {
        match build_translation(config.book_lang, config.dest_dir) {
          Ok(_) => (),
          Err(err) => panic!("::endgroup::Error building translation: {}", err)
        }
      } else {
        update_translation(config.book_lang, config.dest_dir);
      }
    },
    "all" => translate_all(),
    _ => panic!("Supported commands are `build`, `update` and `all`")
  };

}
