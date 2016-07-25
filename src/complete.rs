use std::path::PathBuf;

pub trait Completer {
    fn completions(&self, start: &str) -> Vec<String>;
}

pub struct BasicCompleter {
    prefixes: Vec<String>,
}

impl BasicCompleter {
    pub fn new<T: Into<String>>(prefixes: Vec<T>) -> BasicCompleter {
        BasicCompleter { prefixes: prefixes.into_iter().map(|s| s.into()).collect() }
    }
}

impl Completer for BasicCompleter {
    fn completions(&self, start: &str) -> Vec<String> {
        self.prefixes.iter().filter(|s| s.starts_with(start)).cloned().collect()
    }
}

pub struct FilenameCompleter {
    working_dir: Option<PathBuf>,
}

impl FilenameCompleter {
    pub fn new<T: Into<PathBuf>>(working_dir: Option<T>) -> Self {
        FilenameCompleter { working_dir: working_dir.map(|p| p.into()) }
    }
}

impl Completer for FilenameCompleter {
    fn completions(&self, mut start: &str) -> Vec<String> {
        // XXX: this function is really bad, TODO rewrite

        let start_owned;
        if start.starts_with("\"") || start.starts_with("'") {
            start = &start[1..];
            if start.len() >= 1 {
                start = &start[..start.len() - 1];
            }
            start_owned = start.into();
        } else {
            start_owned = start.replace("\\ ", " ");
        }

        let full_path;
        let start_path = PathBuf::from(&start_owned[..]);

        if let Some(ref wd) = self.working_dir {
            let mut fp = PathBuf::from(wd);
            fp.push(start_owned.clone());
            full_path = fp;
        } else {
            full_path = PathBuf::from(start_owned.clone());
        }

        if full_path.is_relative() {
            return vec![];
        }

        let p;
        let start_name;
        let completing_dir;
        match full_path.parent() {
            // XXX non-unix separaor
            Some(parent) if start != "" && !start_owned.ends_with("/") &&
                            !full_path.ends_with("..") => {
                p = PathBuf::from(parent);
                start_name = full_path.file_name().unwrap().to_string_lossy().into_owned();
                completing_dir = false;
            }
            _ => {
                p = full_path.clone();
                start_name = "".into();
                completing_dir = start == "" || start.ends_with("/") || full_path.ends_with("..");
            }
        }


        let read_dir = match p.read_dir() {
            Ok(x) => x,
            Err(_) => return vec![],
        };

        let mut matches = vec![];
        for dir in read_dir {
            let dir = match dir {
                Ok(x) => x,
                Err(_) => continue,
            };
            let file_name = dir.file_name();
            let file_name = file_name.to_string_lossy();

            if start_name == "" || file_name.starts_with(&*start_name) {
                let mut a = start_path.clone();
                if !a.is_absolute() {
                    a = PathBuf::new();
                } else if !completing_dir && !a.pop() {
                    return vec![];
                }
                a.push(dir.file_name());
                let mut s = a.to_string_lossy().into_owned();
                if dir.path().is_dir() {
                    s = s + "/";
                }

                let mut b = PathBuf::from(start_owned.clone());
                if !completing_dir {
                    b.pop();
                }
                b.push(s);

                matches.push(b.to_string_lossy().to_owned().replace(" ", "\\ "));
            }
        }

        matches
    }
}
