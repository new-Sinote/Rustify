; Rustify per-user installer.
; Build from this directory: makensis rustify.nsi
; The output is written to ..\dist\.

Unicode true
ManifestDPIAware true
SetCompressor /SOLID lzma
SetCompressorDictSize 64

!include "MUI2.nsh"
!include "FileFunc.nsh"

!define APP_NAME "Rustify"
!define APP_VERSION "0.3.0"
!define APP_PUBLISHER "Rustify contributors"
!define APP_EXE "rustify.exe"
!define APP_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Rustify"
!define APP_DATA "$LOCALAPPDATA\Rustify"

Name "${APP_NAME} ${APP_VERSION}"
BrandingText "${APP_NAME}"
OutFile "..\dist\${APP_NAME}-${APP_VERSION}-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\${APP_NAME}"
InstallDirRegKey HKCU "Software\${APP_NAME}" "InstallDir"
RequestExecutionLevel user

VIProductVersion "0.3.0.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"
VIAddVersionKey "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey "FileDescription" "${APP_NAME} installer"
VIAddVersionKey "FileVersion" "0.3.0.0"
VIAddVersionKey "LegalCopyright" "MIT (Rustify); GPL-3.0 (uBlock Origin Lite)"

!define MUI_ICON "..\assets\rustify.ico"
!define MUI_UNICON "..\assets\rustify.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Launch Rustify"
!define MUI_FINISHPAGE_LINK "Rustify on GitHub"
!define MUI_FINISHPAGE_LINK_LOCATION "https://github.com/new-Sinote/Rustify"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE"
!insertmacro MUI_PAGE_LICENSE "ublock-payload\LICENSE.txt"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Rustify" SecMain
  SetShellVarContext current
  SetOutPath "$INSTDIR"

  ; Refuse to overwrite a running copy instead of failing halfway through.
  IfFileExists "$INSTDIR\${APP_EXE}" 0 install
  ClearErrors
  Delete "$INSTDIR\${APP_EXE}"
  IfErrors 0 install
  MessageBox MB_OK|MB_ICONSTOP "Rustify is still running. Quit it from the tray menu, then run this installer again."
  Abort

  install:
  File "..\target\release\${APP_EXE}"
  File "/oname=LICENSE.txt" "..\..\LICENSE"

  SetOutPath "$INSTDIR\extensions\ublock"
  File /r "ublock-payload\*.*"
  SetOutPath "$INSTDIR"
  File "/oname=UBOL_LICENSE.txt" "ublock-payload\LICENSE.txt"
  File "/oname=UBOL_SOURCE.txt" "ublock-payload\SOURCE.txt"

  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortCut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
  CreateShortCut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"

  WriteRegStr HKCU "Software\${APP_NAME}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${APP_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${APP_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "${APP_KEY}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKCU "${APP_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "${APP_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${APP_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr HKCU "${APP_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${APP_KEY}" "URLInfoAbout" "https://github.com/new-Sinote/Rustify"
  WriteRegDWORD HKCU "${APP_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${APP_KEY}" "NoRepair" 1

  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "${APP_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "Uninstall"
  SetShellVarContext current

  Delete "$DESKTOP\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"

  DeleteRegKey HKCU "${APP_KEY}"
  DeleteRegKey HKCU "Software\${APP_NAME}"

  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\UBOL_LICENSE.txt"
  Delete "$INSTDIR\UBOL_SOURCE.txt"
  RMDir /r "$INSTDIR\extensions"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  IfSilent keep_data
  MessageBox MB_YESNO|MB_ICONQUESTION \
    "Also remove your Rustify browser profile and settings?$\r$\n$\r$\nThis signs you out of Spotify and deletes the local WebView2 data in:$\r$\n${APP_DATA}" \
    IDNO keep_data
  RMDir /r "${APP_DATA}"

  keep_data:
SectionEnd
