!macro NSIS_HOOK_PREUNINSTALL
  IfSilent scoopui_uninstall_done
  StrCmp $UpdateMode 1 scoopui_uninstall_done
  StrCmp $PassiveMode 1 scoopui_uninstall_done

  ReadRegStr $0 HKCU "Software\ScoopUI" "ScoopDir"
  StrCmp $0 "" scoopui_uninstall_done

  StrCmp $0 $INSTDIR scoopui_uninstall_done

  MessageBox MB_YESNO|MB_ICONEXCLAMATION "检测到 Scoop 环境与数据：$\r$\n$0$\r$\n$\r$\n其中包含已安装的应用、缓存与持久化数据（不可恢复）。$\r$\n是否连同该目录一起删除？" IDNO scoopui_uninstall_done

  nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -Command "$$s = $\'$0\shims$\'.TrimEnd($\'\$\'); $$p = [Environment]::GetEnvironmentVariable($\'Path$\', $\'User$\'); $$n = (($$p -split $\';$\') | Where-Object { $$_.Trim() -ne $\'$\' -and $$_.Trim().TrimEnd($\'\$\') -ne $$s }) -join $\';$\'; [Environment]::SetEnvironmentVariable($\'Path$\', $$n, $\'User$\'); [Environment]::SetEnvironmentVariable($\'SCOOP$\', $$null, $\'User$\')"'

  RMDir /r "$0"
  DeleteRegValue HKCU "Software\ScoopUI" "ScoopDir"
  DeleteRegValue HKCU "Software\ScoopUI" "BootRoot"

  scoopui_uninstall_done:
!macroend
