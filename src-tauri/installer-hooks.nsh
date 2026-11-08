; DxTool additions to Tauri's Windows installer (see tauri.conf.json → bundle.windows.nsis).

; After installing: offer to connect Claude Desktop (normal or Microsoft Store version).
; Skipped for quiet installs and updates.
!macro NSIS_HOOK_POSTINSTALL
  ${If} $UpdateMode <> 1
  ${AndIf} $PassiveMode <> 1
  ${AndIfNot} ${Silent}
    StrCpy $0 0
    ${If} ${FileExists} "$APPDATA\Claude\*.*"
      StrCpy $0 1
    ${EndIf}
    ClearErrors
    FindFirst $1 $2 "$LOCALAPPDATA\Packages\Claude_*"
    ${If} $2 != ""
      StrCpy $0 1
    ${EndIf}
    FindClose $1
    ${If} $0 = 1
      ${If} ${Cmd} `MessageBox MB_YESNO|MB_ICONQUESTION "Connect DxTool to Claude Desktop?$\r$\n$\r$\nClaude will then be able to run DxTool's health scans and read the results, so you can ask it about this PC in plain English. Claude can't run repairs or delete files.$\r$\n$\r$\nYou can change this later in DxTool's Settings." IDYES`
        ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --setup-claude' $0
        ${If} $0 = 0
          MessageBox MB_OK|MB_ICONINFORMATION "DxTool is connected to Claude Desktop.$\r$\n$\r$\nIf Claude Desktop is open, quit it completely (right-click its icon by the clock, then Quit) and open it again."
        ${Else}
          MessageBox MB_OK|MB_ICONEXCLAMATION "DxTool couldn't connect to Claude Desktop. You can try again in DxTool: Settings, Claude AI assistant."
        ${EndIf}
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend

; Before uninstalling (not when updating): disconnect Claude Desktop if it points to this copy.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --remove-claude'
  ${EndIf}
!macroend

; After uninstalling: if "Delete the application data" was ticked, also remove DxTool's own
; data folder (saved scans, reports, settings, ClamAV), which lives next to the program.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    RMDir /r "$INSTDIR\DxTool-Data"
    RMDir "$INSTDIR"
  ${EndIf}
!macroend
