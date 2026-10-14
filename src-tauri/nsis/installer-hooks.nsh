!macro NSIS_HOOK_PREUNINSTALL
  ReadRegStr $0 HKCU "Software\ScoopUI" "BootRoot"
  StrCmp $0 "" scoopui_uninstall_done
  MessageBox MB_YESNO|MB_ICONEXCLAMATION "检测到由 ScoopUI 初始化的 Scoop 环境：$\r$\n$0$\r$\n$\r$\n是否连同其中的已安装应用一起删除？（不可恢复）$\r$\n（如已将 scoop 加入 PATH，请随后在系统环境变量中移除其 shims 条目）" IDNO scoopui_uninstall_done
  RMDir /r "$0"
  DeleteRegValue HKCU "Software\ScoopUI" "BootRoot"
  scoopui_uninstall_done:
!macroend
