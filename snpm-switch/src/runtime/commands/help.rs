pub(super) fn print_switch_help() {
    println!("snpm-switch - Version manager for snpm\n");
    println!("Usage:");
    println!(
        "  snpm-switch [--switch-version <version> | --switch-ignore-package-manager] <snpm-command>"
    );
    println!("  snpm-switch switch list    List cached snpm versions");
    println!("  snpm-switch switch cache   Cache project/default version or explicit versions");
    println!("  snpm-switch switch which   Print the binary path that would be executed");
    println!("  snpm-switch switch clear   Clear the version cache");
    println!();
    println!("Switch flags:");
    println!("  --switch-version <version>         Override the project packageManager");
    println!("  --switch-ignore-package-manager    Ignore the project packageManager");
}
