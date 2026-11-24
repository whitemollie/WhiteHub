; Remove the per-user autostart entry (and Task Manager's enable/disable flag for it),
; so no dangling startup item remains after uninstalling.
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "WhiteHub"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "WhiteHub"
!macroend
