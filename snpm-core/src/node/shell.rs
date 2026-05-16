use crate::{Result, SnpmError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellFlavor {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

impl ShellFlavor {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "bash" => Ok(ShellFlavor::Bash),
            "zsh" => Ok(ShellFlavor::Zsh),
            "fish" => Ok(ShellFlavor::Fish),
            "powershell" | "pwsh" | "posh" => Ok(ShellFlavor::PowerShell),
            other => Err(SnpmError::Internal {
                reason: format!("unsupported shell '{other}' (use bash, zsh, fish, or powershell)"),
            }),
        }
    }

    pub fn detect() -> Result<Self> {
        if cfg!(windows) {
            return Ok(ShellFlavor::PowerShell);
        }

        if let Ok(shell) = std::env::var("SHELL") {
            if shell.contains("fish") {
                return Ok(ShellFlavor::Fish);
            }
            if shell.contains("zsh") {
                return Ok(ShellFlavor::Zsh);
            }
            if shell.contains("bash") {
                return Ok(ShellFlavor::Bash);
            }
        }

        Ok(ShellFlavor::Bash)
    }
}

pub fn shell_init_script(flavor: ShellFlavor) -> String {
    match flavor {
        ShellFlavor::Bash | ShellFlavor::Zsh => posix_script(flavor),
        ShellFlavor::Fish => fish_script(),
        ShellFlavor::PowerShell => powershell_script(),
    }
}

fn posix_script(flavor: ShellFlavor) -> String {
    let hook = if matches!(flavor, ShellFlavor::Zsh) {
        ZSH_HOOK
    } else {
        BASH_HOOK
    };

    format!(
        "# snpm node shell integration\n{COMMON_POSIX}\n{hook}\n_snpm_node_apply\n",
        COMMON_POSIX = COMMON_POSIX
    )
}

const COMMON_POSIX: &str = r#"_snpm_node_apply() {
  if ! command -v snpm >/dev/null 2>&1; then
    return 0
  fi
  local bin
  bin="$(snpm node which --active --quiet 2>/dev/null)"
  if [ -z "$bin" ]; then
    return 0
  fi
  case ":$PATH:" in
    *:"$bin":*) ;;
    *) PATH="$bin:$PATH"; export PATH ;;
  esac
}"#;

const BASH_HOOK: &str = r#"_snpm_node_prompt() {
  _snpm_node_apply
}
case "${PROMPT_COMMAND-}" in
  *_snpm_node_prompt*) ;;
  *) PROMPT_COMMAND="_snpm_node_prompt${PROMPT_COMMAND:+;$PROMPT_COMMAND}" ;;
esac"#;

const ZSH_HOOK: &str = r#"autoload -Uz add-zsh-hook 2>/dev/null
if typeset -f add-zsh-hook >/dev/null; then
  add-zsh-hook chpwd _snpm_node_apply
  add-zsh-hook precmd _snpm_node_apply
fi"#;

fn fish_script() -> String {
    r#"# snpm node shell integration
function _snpm_node_apply --on-variable PWD --on-event fish_prompt
    command -v snpm >/dev/null 2>&1; or return 0
    set -l bin (snpm node which --active --quiet 2>/dev/null)
    if test -n "$bin"; and not contains -- $bin $PATH
        set -x PATH $bin $PATH
    end
end
_snpm_node_apply
"#
    .to_string()
}

fn powershell_script() -> String {
    r#"# snpm node shell integration
function global:Invoke-SnpmNodeApply {
    if (-not (Get-Command snpm -ErrorAction SilentlyContinue)) { return }
    $bin = (& snpm node which --active --quiet) 2>$null
    if ([string]::IsNullOrWhiteSpace($bin)) { return }
    if (-not ($env:PATH -split [IO.Path]::PathSeparator | Where-Object { $_ -eq $bin })) {
        $env:PATH = "$bin" + [IO.Path]::PathSeparator + $env:PATH
    }
}
$global:SnpmNodePromptOriginal = $function:prompt
function global:prompt {
    Invoke-SnpmNodeApply
    if ($global:SnpmNodePromptOriginal) {
        & $global:SnpmNodePromptOriginal
    } else {
        "PS $($executionContext.SessionState.Path.CurrentLocation)> "
    }
}
Invoke-SnpmNodeApply
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::{ShellFlavor, shell_init_script};

    #[test]
    fn parses_known_shells() {
        assert_eq!(ShellFlavor::parse("bash").unwrap(), ShellFlavor::Bash);
        assert_eq!(ShellFlavor::parse("ZSH").unwrap(), ShellFlavor::Zsh);
        assert_eq!(ShellFlavor::parse("fish").unwrap(), ShellFlavor::Fish);
        assert_eq!(ShellFlavor::parse("pwsh").unwrap(), ShellFlavor::PowerShell);
        assert!(ShellFlavor::parse("nu").is_err());
    }

    #[test]
    fn posix_scripts_call_snpm_node_which() {
        let bash = shell_init_script(ShellFlavor::Bash);
        assert!(bash.contains("snpm node which --active --quiet"));
        assert!(bash.contains("PROMPT_COMMAND"));

        let zsh = shell_init_script(ShellFlavor::Zsh);
        assert!(zsh.contains("snpm node which --active --quiet"));
        assert!(zsh.contains("add-zsh-hook"));
    }

    #[test]
    fn fish_script_uses_on_variable() {
        let fish = shell_init_script(ShellFlavor::Fish);
        assert!(fish.contains("--on-variable PWD"));
    }

    #[test]
    fn powershell_script_overrides_prompt() {
        let pwsh = shell_init_script(ShellFlavor::PowerShell);
        assert!(pwsh.contains("function global:prompt"));
    }
}
