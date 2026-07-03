; NSIS installer hooks for OpenStart
; Creates a desktop shortcut on install

!macro NSIS_HOOK_POSTINSTALL
  CreateShortcut "$DESKTOP\OpenStart.lnk" "$INSTDIR\openstart.exe"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$DESKTOP\OpenStart.lnk"
!macroend
