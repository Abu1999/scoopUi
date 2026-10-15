; ScoopUI 卸载前清理钩子。
;
; 设计目标：卸载后不留任何痕迹——
;   1. 各应用自带的 uninstall-*.reg（右键菜单 / 文件关联）必须回放；
;   2. 用户 PATH 中指向该环境的条目（shims 与各应用目录）、SCOOP 变量、
;      .config\scoop\config.json 的 root_path、开始菜单「Scoop Apps」快捷方式；
;   3. 应用自身数据与注册表：$INSTDIR\data、%APPDATA%\scoopui、%LOCALAPPDATA%\scoopui、
;      HKCU\Software\ScoopUI 整键。
;
; 安全前提：Tauri 覆盖安装时会以 UpdateMode 调用本卸载器，此时必须原样保留数据。

!macro NSIS_HOOK_PREUNINSTALL
  ; 覆盖安装（升级）：什么都不动，避免每次升级丢配置。
  StrCmp $UpdateMode 1 scoopui_uninstall_done

  ; --- 解析 Scoop 环境目录（ScoopDir 优先，兼容旧 BootRoot） ---
  ReadRegStr $0 HKCU "Software\ScoopUI" "ScoopDir"
  ReadRegStr $1 HKCU "Software\ScoopUI" "BootRoot"
  StrCmp $0 "" 0 scoopui_have_dir
  StrCpy $0 $1
  scoopui_have_dir:
  StrCmp $0 "" scoopui_cleanup_traces

  ; 环境目录位于安装目录之内：随程序一起清理，无需询问。
  StrLen $2 $INSTDIR
  StrCpy $3 $0 $2
  StrCmp $3 $INSTDIR scoopui_purge_env

  ; 交互式卸载时询问是否连环境目录一起删。
  IfSilent scoopui_cleanup_traces
  StrCmp $PassiveMode 1 scoopui_cleanup_traces
  MessageBox MB_YESNO|MB_ICONEXCLAMATION "检测到 Scoop 环境目录：$\r$\n$0$\r$\n$\r$\n其中包含已安装的应用、缓存与持久化数据（不可恢复）。$\r$\n是否连同该目录一起删除？" IDNO scoopui_cleanup_traces

  scoopui_purge_env:
  ; 1) 回放各应用的 uninstall-*.reg，清掉「Git Bash Here」这类右键菜单与文件关联
  ;    （应用根、current 与各版本子目录内都可能有回滚脚本）。
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-ChildItem -Path $\'$0\apps\*\uninstall-*.reg$\', $\'$0\apps\*\*\uninstall-*.reg$\' -ErrorAction SilentlyContinue | ForEach-Object { & reg.exe import $$_.FullName }"'

  ; 2) 从用户 PATH 去掉指向该环境的所有条目（shims 与各应用目录），并删除 SCOOP 变量。
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -Command "$$root = $\'$0$\'.TrimEnd($\'\$\'); $$p = [Environment]::GetEnvironmentVariable($\'Path$\', $\'User$\'); $$n = (($$p -split $\';$\') | Where-Object { $$t = $$_.Trim().TrimEnd($\'\$\'); $$t -ne $\'$\' -and $$t -ine $$root -and -not $$t.StartsWith($$root + $\'\$\', [System.StringComparison]::OrdinalIgnoreCase) }) -join $\';$\'; $$k = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($\'Environment$\', $$true); if ($$n) { $$k.SetValue($\'Path$\', $$n, [Microsoft.Win32.RegistryValueKind]::ExpandString) } else { $$k.DeleteValue($\'Path$\', $$false) }; $$k.Close(); [Environment]::SetEnvironmentVariable($\'SCOOP$\', $$null, $\'User$\')"'

  ; 3) 清掉 %USERPROFILE%\.config\scoop\config.json 里的 root_path。
  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -Command "$$f = Join-Path $$env:USERPROFILE $\'.config\scoop\config.json$\'; if (Test-Path -LiteralPath $$f) { $$j = Get-Content -LiteralPath $$f -Raw | ConvertFrom-Json; $$q = $$j.PSObject.Properties | Where-Object { $$_.Name -ieq $\'root_path$\' } | Select-Object -First 1; if ($$q) { $$j.PSObject.Properties.Remove($$q.Name); [IO.File]::WriteAllText($$f, ($$j | ConvertTo-Json -Depth 20)) } }"'

  ; 4) 删除开始菜单「Scoop Apps」快捷方式（指向即将被删除的应用）。
  RMDir /r "$APPDATA\Microsoft\Windows\Start Menu\Programs\Scoop Apps"

  RMDir /r "$0"

  ; 环境目录仍有残留（文件被占用，如资源管理器加载的扩展 DLL）时给出提示（仅交互模式）。
  IfFileExists "$0\*.*" 0 scoopui_env_clean
  IfSilent scoopui_env_clean
  StrCmp $PassiveMode 1 scoopui_env_clean
  MessageBox MB_OK|MB_ICONEXCLAMATION "部分文件被其他程序占用，未能删除：$\r$\n$0$\r$\n$\r$\n请在重启资源管理器（或重启电脑）后手动删除该目录。"

  scoopui_env_clean:

  scoopui_cleanup_traces:
  ; 无论是否删除环境目录，都清掉应用自身的数据与注册表痕迹。
  RMDir /r "$INSTDIR\data"
  RMDir /r "$APPDATA\scoopui"
  RMDir /r "$LOCALAPPDATA\scoopui"
  DeleteRegKey HKCU "Software\ScoopUI"

  scoopui_uninstall_done:
!macroend
