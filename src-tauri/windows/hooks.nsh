; Autostart across updates and uninstalling.
;
; The installer updates by running the old uninstaller without /UPDATE. It then runs in place;
; a real uninstall runs from a temporary copy. Tauri's template removes the Run entry in both cases.
; - Update: remember the Run entry and restore it after installing; keep Task Manager's
;   enable/disable flag, so an entry the user disabled there stays disabled.
; - Real uninstall: remove the flag too, so nothing is left behind.
!define WHITEHUB_RUN "Software\Microsoft\Windows\CurrentVersion\Run"
!define WHITEHUB_APPROVED "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"

!macro NSIS_HOOK_PREUNINSTALL
  Push $0
  DeleteRegValue HKCU "${MANUPRODUCTKEY}" "RestoreAutostart"
  ${If} $EXEPATH == "$INSTDIR\uninstall.exe"
    ReadRegStr $0 HKCU "${WHITEHUB_RUN}" "${PRODUCTNAME}"
    ${If} $0 != ""
      WriteRegStr HKCU "${MANUPRODUCTKEY}" "RestoreAutostart" "1"
    ${EndIf}
  ${EndIf}
  Pop $0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $EXEPATH != "$INSTDIR\uninstall.exe"
  ${AndIf} $UpdateMode <> 1
    DeleteRegValue HKCU "${WHITEHUB_RUN}" "${PRODUCTNAME}"
    DeleteRegValue HKCU "${WHITEHUB_APPROVED}" "${PRODUCTNAME}"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Push $0
  ReadRegStr $0 HKCU "${MANUPRODUCTKEY}" "RestoreAutostart"
  ${If} $0 == "1"
    ; Same command as in autostart.rs
    WriteRegStr HKCU "${WHITEHUB_RUN}" "${PRODUCTNAME}" '"$INSTDIR\${MAINBINARYNAME}.exe" --autostart'
    DeleteRegValue HKCU "${MANUPRODUCTKEY}" "RestoreAutostart"
  ${EndIf}
  Pop $0
!macroend
