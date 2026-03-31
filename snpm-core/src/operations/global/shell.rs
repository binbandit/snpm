use crate::console;

use std::path::Path;

pub(super) fn print_path_setup_hint(bin_dir: &Path) {
    let bin_path = bin_dir.display();

    if std::env::var("PATH")
        .map(|path| path.contains(&bin_dir.to_string_lossy().to_string()))
        .unwrap_or(false)
    {
        console::info(&format!("Binaries available at: {}", bin_path));
        return;
    }

    console::info(&format!("Binaries installed to: {}", bin_path));
    println!();
    console::info("Add to PATH by running:");
    println!();

    let shell = std::env::var("SHELL").unwrap_or_default();

    if shell.contains("zsh") {
        println!("  echo 'export PATH=\"{}:$PATH\"' >> ~/.zshrc", bin_path);
        println!("  source ~/.zshrc");
    } else if shell.contains("bash") {
        println!("  echo 'export PATH=\"{}:$PATH\"' >> ~/.bashrc", bin_path);
        println!("  source ~/.bashrc");
    } else if shell.contains("fish") {
        println!("  fish_add_path {}", bin_path);
    } else {
        println!("  export PATH=\"{}:$PATH\"", bin_path);
    }
}
