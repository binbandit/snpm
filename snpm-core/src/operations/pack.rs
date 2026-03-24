use crate::{Project, Result, SnpmError};
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tar::Builder;

pub struct PackResult {
    pub tarball_path: PathBuf,
    pub file_count: usize,
    pub size: u64,
    pub name: String,
    pub version: String,
}

pub fn pack(project: &Project, output_dir: &Path) -> Result<PackResult> {
    let name = project
        .manifest
        .name
        .as_deref()
        .ok_or_else(|| SnpmError::ManifestInvalid {
            path: project.manifest_path.clone(),
            reason: "package.json must have a \"name\" field to pack".into(),
        })?;

    let version = project
        .manifest
        .version
        .as_deref()
        .ok_or_else(|| SnpmError::ManifestInvalid {
            path: project.manifest_path.clone(),
            reason: "package.json must have a \"version\" field to pack".into(),
        })?;

    let safe_name = name.replace('/', "-").replace('@', "");
    let tarball_name = format!("{}-{}.tgz", safe_name, version);
    let tarball_path = output_dir.join(&tarball_name);

    let files = collect_pack_files(project)?;

    let tar_data = Vec::new();
    let mut builder = Builder::new(tar_data);

    for file_path in &files {
        let rel_path = file_path
            .strip_prefix(&project.root)
            .unwrap_or(file_path);
        let archive_path = Path::new("package").join(rel_path);

        let metadata = fs::metadata(file_path).map_err(|source| SnpmError::ReadFile {
            path: file_path.clone(),
            source,
        })?;

        if metadata.is_file() {
            let data = fs::read(file_path).map_err(|source| SnpmError::ReadFile {
                path: file_path.clone(),
                source,
            })?;

            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(0);
            header.set_cksum();

            builder
                .append_data(&mut header, &archive_path, data.as_slice())
                .map_err(|source| SnpmError::Archive {
                    path: tarball_path.clone(),
                    source,
                })?;
        }
    }

    let tar_bytes = builder.into_inner().map_err(|e| SnpmError::Archive {
        path: tarball_path.clone(),
        source: std::io::Error::other(e.to_string()),
    })?;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&tar_bytes)
        .map_err(|source| SnpmError::WriteFile {
            path: tarball_path.clone(),
            source,
        })?;

    let compressed = encoder.finish().map_err(|source| SnpmError::WriteFile {
        path: tarball_path.clone(),
        source,
    })?;

    fs::create_dir_all(output_dir).map_err(|source| SnpmError::WriteFile {
        path: output_dir.to_path_buf(),
        source,
    })?;

    fs::write(&tarball_path, &compressed).map_err(|source| SnpmError::WriteFile {
        path: tarball_path.clone(),
        source,
    })?;

    Ok(PackResult {
        size: compressed.len() as u64,
        file_count: files.len(),
        tarball_path,
        name: name.to_string(),
        version: version.to_string(),
    })
}

fn collect_pack_files(project: &Project) -> Result<Vec<PathBuf>> {
    let root = &project.root;

    let mut files = vec![project.manifest_path.clone()];

    if let Some(ref file_patterns) = project.manifest.files {
        for pattern in file_patterns {
            let full_pattern = root.join(pattern);
            let pattern_str = full_pattern.to_string_lossy();

            match glob::glob(&pattern_str) {
                Ok(paths) => {
                    for entry in paths.flatten() {
                        if entry.is_file() && entry != project.manifest_path {
                            files.push(entry);
                        } else if entry.is_dir() {
                            collect_dir_files(&entry, &mut files)?;
                        }
                    }
                }
                Err(_) => {
                    let literal = root.join(pattern);
                    if literal.is_file() && literal != project.manifest_path {
                        files.push(literal);
                    } else if literal.is_dir() {
                        collect_dir_files(&literal, &mut files)?;
                    }
                }
            }
        }
    } else {
        collect_dir_files_filtered(root, &mut files, project)?;
    }

    for mandatory in &["README", "LICENSE", "LICENCE", "CHANGELOG"] {
        for entry in fs::read_dir(root)
            .map_err(|source| SnpmError::ReadFile {
                path: root.clone(),
                source,
            })?
            .flatten()
        {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.to_uppercase().starts_with(mandatory)
                && entry.path().is_file()
                && !files.contains(&entry.path())
            {
                files.push(entry.path());
            }
        }
    }

    if let Some(ref main) = project.manifest.main {
        let main_path = root.join(main);
        if main_path.is_file() && !files.contains(&main_path) {
            files.push(main_path);
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_dir_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)
        .map_err(|source| SnpmError::ReadFile {
            path: dir.to_path_buf(),
            source,
        })?
        .flatten()
    {
        let path = entry.path();
        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            collect_dir_files(&path, files)?;
        }
    }
    Ok(())
}

fn collect_dir_files_filtered(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    project: &Project,
) -> Result<()> {
    let npmignore_path = project.root.join(".npmignore");
    let gitignore_path = project.root.join(".gitignore");

    let ignore_patterns = if npmignore_path.is_file() {
        fs::read_to_string(&npmignore_path)
            .ok()
            .map(|content| parse_ignore_patterns(&content))
    } else if gitignore_path.is_file() {
        fs::read_to_string(&gitignore_path)
            .ok()
            .map(|content| parse_ignore_patterns(&content))
    } else {
        None
    };

    collect_dir_recursive(dir, files, &project.root, ignore_patterns.as_deref())
}

fn collect_dir_recursive(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    root: &Path,
    ignore_patterns: Option<&[String]>,
) -> Result<()> {
    for entry in fs::read_dir(dir)
        .map_err(|source| SnpmError::ReadFile {
            path: dir.to_path_buf(),
            source,
        })?
        .flatten()
    {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if matches!(
            name_str.as_ref(),
            "node_modules"
                | ".git"
                | ".snpm"
                | ".DS_Store"
                | "snpm-lock.yaml"
                | "pnpm-lock.yaml"
                | "package-lock.json"
        ) {
            continue;
        }

        if let Some(patterns) = ignore_patterns {
            let relative_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy();
            if patterns.iter().any(|pattern| matches_ignore_pattern(&relative_path, pattern)) {
                continue;
            }
        }

        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            collect_dir_recursive(&path, files, root, ignore_patterns)?;
        }
    }
    Ok(())
}

fn parse_ignore_patterns(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect()
}

fn matches_ignore_pattern(path: &str, pattern: &str) -> bool {
    let pattern = pattern.trim_end_matches('/');

    if !pattern.contains('/') {
        return path
            .split('/')
            .any(|component| component == pattern);
    }

    path.starts_with(pattern) || path == pattern
}
