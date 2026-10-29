; Sift Launcher — Windows installer.
;
; A per-user install: no administrator, no UAC, no machine-wide state. The
; launcher goes under %LOCALAPPDATA%\Programs\Sift Launcher, and its data
; (instances, worlds, settings) stays where the launcher already keeps it,
; %APPDATA%\SiftLauncher — the uninstaller asks before touching that.
;
; Build (from the workspace root):
;   cargo build --release --locked -p sift-desktop
;   python3 tools/gen_installer_anim.py            # the header animation
;   makensis installer/sift.nsi                    # -> dist/SiftLauncher-<ver>-x64-setup.exe
;
; VERSION comes from Cargo.toml in CI so the installer cannot claim a version the
; binary is not, and APP_EXE is overridable so the script can be compiled on a
; machine that has not built the Windows target yet.
;
; The first page plays a generated AVI of the launcher's mark turning. It is an
; uncompressed AVI rather than a GIF or an APNG because the one control Windows
; ships for animation (`SysAnimate32`) reads exactly that, on a machine with
; nothing else installed — which is the machine an installer runs on.

Unicode true
ManifestDPIAware true
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 64

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"

!ifndef VERSION
  ; Kept in step with `crates/sift-ui/Cargo.toml`, which is the manifest the release tag is
  ; checked against; a release step may override it with /DVERSION=.
  !define VERSION "0.1.0"
!endif
; The default is the product's own build output. `crates/sift-ui` is its own workspace, so
; cargo writes there rather than to the root `target/`; the file inside is `SiftUi.exe` and the
; installer renames it to `SiftLauncher.exe` on install, which is the name the Start menu, the
; uninstall entry and the shortcut all use.
; The exe to install, as one whole path. Absolute when the caller can make it absolute:
; composing it from `${HERE}` and a relative name is what failed twice on the runner's NSIS 3.13
; (`...\installer\dist/SiftUi.exe`), and a path that needs no joining cannot fail that way. The
; default is for a local build from the repository root.
!ifndef PAYLOAD
  !define PAYLOAD "${__FILEDIR__}/../crates/sift-ui/target/release/SiftUi.exe"
!endif

; The script's own directory, with the trailing separator every path below assumes.
; `__FILEDIR__` does not end with one on NSIS 3.13, so `${HERE}sift.ico` resolved to
; `installersift.ico` and makensis aborted on the icon line.
!define HERE "${__FILEDIR__}\"

!define PRODUCT "Sift Launcher"
!define PUBLISHER "Palantir Studios"
; The launcher's own data folder, which is a product decision the launcher
; already made: `sift-core`'s data root is %APPDATA%\<name>. It is *not*
; under $INSTDIR, so an uninstall that removed it would delete worlds.
!define DATA_DIR "$APPDATA\SiftLauncher"
!define APP_KEY "Software\SiftLauncher"
!define APP_PATH_KEY "Software\Microsoft\Windows\CurrentVersion\App Paths\Sift Launcher.exe"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\SiftLauncher"

; The window and animate styles come with NSIS (`ACS_CENTER` and friends are
; defined by the headers MUI2 pulls in); the two `ACM_*` messages do not, and an
; undefined constant is a compile error rather than a wrong value, so they are
; written here as the numbers Windows documents.


Name "${PRODUCT} ${VERSION}"
BrandingText "${PRODUCT} ${VERSION}"
OutFile "${HERE}../dist/SiftLauncher-${VERSION}-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\Sift Launcher"
InstallDirRegKey HKCU "${APP_KEY}" "InstallDir"
Icon "${HERE}sift.ico"
UninstallIcon "${HERE}sift.ico"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${PRODUCT}"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "FileDescription" "${PRODUCT} installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Copyright (c) 2026 ${PUBLISHER}"

!define MUI_ABORTWARNING
!define MUI_ICON "${HERE}sift.ico"
!define MUI_UNICON "${HERE}sift.ico"
; Checked by default: a first install is finished when the launcher opens, not
; when a wizard closes. Nothing runs in a silent install.
!define MUI_FINISHPAGE_RUN "$INSTDIR\Sift Launcher.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start ${PRODUCT}"

; The animated page first, then the stock wizard: the mark is what the
; installer is, and the licence and folder choices come after it.
Page custom ShowMark
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${HERE}../LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "German"
!insertmacro MUI_LANGUAGE "French"
!insertmacro MUI_LANGUAGE "Japanese"

; ---- The animated page -------------------------------------------------
;
; A page of its own rather than a bitmap on the welcome page, because the
; animation is the one thing here that has to *run*: `SysAnimate32` is a Windows
; common control, and the wizard's stock pages have no place to put one.
;
; The AVI is written to $PLUGINSDIR (removed with the installer) rather than to
; $INSTDIR, because a file this installer draws with is not one it means to
; leave behind.
Function ShowMark
  !insertmacro MUI_HEADER_TEXT "${PRODUCT}" "A Minecraft launcher for Windows."

  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}

  ; shortcut: no animation on the welcome page, upgrade if the banner is wanted back.
  ;
  ; This page used to open `anim/sift.avi` in a `SysAnimate32` control. POSIX makensis compiles
  ; that happily and the GitHub runner's NSIS 3.13 aborts on it, which cost three releases
  ; (0.1.1, 0.1.2, 0.1.4) before the line was blamed. A decoration is not worth another one: the
  ; page's text is what a reader reads, and the installer now compiles wherever makensis runs.
  ; If the banner is ever wanted back, draw it as a bitmap with `NSD_CreateBitmap`, which no
  ; version of NSIS has ever refused.

  ${NSD_CreateLabel} 0 104u 100% 60u "Sift browses Modrinth and CurseForge, installs modpacks, and launches both editions of Minecraft.$\r$\n$\r$\nThis installs the launcher for your Windows account only — no administrator password, no files outside your user folder.$\r$\n$\r$\nNot affiliated with Mojang, Microsoft, Modrinth or Overwolf."
  Pop $2

  nsDialogs::Show
FunctionEnd

Function .onInit
  ; The desktop builds only for x86_64. Refusing here is clearer than a launched
  ; app that fails to start on a 32-bit system.
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP|MB_OK "${PRODUCT} is a 64-bit application and cannot run on this 32-bit copy of Windows."
    Abort
  ${EndIf}
  SetShellVarContext current
  SetRegView 64

  ; An upgrade over a running launcher cannot replace the exe. Say so before the
  ; wizard collects choices, rather than failing on the last page mid-copy.
  IfFileExists "$INSTDIR\Sift Launcher.exe" 0 done
  ClearErrors
  FileOpen $0 "$INSTDIR\Sift Launcher.exe" a
  ${If} ${Errors}
    MessageBox MB_ICONSTOP|MB_OK "${PRODUCT} is running. Close it and run this installer again."
    Abort
  ${EndIf}
  FileClose $0
  done:
FunctionEnd

Section "${PRODUCT} (required)" SEC_APP
  SectionIn RO
  SetShellVarContext current
  SetRegView 64
  SetOutPath "$INSTDIR"

  ; The installed name is fixed here, not taken from whatever the payload
  ; happened to be called: the shortcuts, the App Paths entry and the uninstaller
  ; all name "Sift Launcher.exe".
  ; Copied under a space-free name and then renamed: `/oname=` takes a bare
  ; token, so a destination filename containing a space cannot be written on the
  ; `File` line itself, and the installed name is the one the shortcuts and the
  ; uninstaller both depend on.
  File /oname=SiftLauncher.exe "${PAYLOAD}"
  Rename "$INSTDIR\SiftLauncher.exe" "$INSTDIR\Sift Launcher.exe"
  File "${HERE}sift.ico"
  File "${HERE}../LICENSE"
  File "${HERE}../THIRD_PARTY_NOTICES.md"

  CreateDirectory "$SMPROGRAMS\${PRODUCT}"
  CreateShortcut "$SMPROGRAMS\${PRODUCT}\${PRODUCT}.lnk" "$INSTDIR\Sift Launcher.exe" "" "$INSTDIR\sift.ico" 0
  CreateShortcut "$SMPROGRAMS\${PRODUCT}\Uninstall ${PRODUCT}.lnk" "$INSTDIR\uninstall.exe"

  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Where an upgrade finds the install, and what Win+R resolves.
  WriteRegStr HKCU "${APP_KEY}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${APP_KEY}" "Version" "${VERSION}"
  WriteRegStr HKCU "${APP_PATH_KEY}" "" "$INSTDIR\Sift Launcher.exe"
  WriteRegStr HKCU "${APP_PATH_KEY}" "Path" "$INSTDIR"

  ; Add or Remove Programs.
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\sift.ico"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" $0
SectionEnd

Section "Desktop shortcut" SEC_DESKTOP
  SetShellVarContext current
  CreateShortcut "$DESKTOP\${PRODUCT}.lnk" "$INSTDIR\Sift Launcher.exe" "" "$INSTDIR\sift.ico" 0
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_APP} "The launcher, its icon and its notices. Instances, worlds and settings stay in your user data folder."
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_DESKTOP} "Put a ${PRODUCT} shortcut on the desktop."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  SetShellVarContext current
  SetRegView 64

  ; A running launcher holds its exe open, and a half-removed install with a dead
  ; uninstaller is worse than asking the player to close it first.
  IfFileExists "$INSTDIR\Sift Launcher.exe" 0 not_running
  ClearErrors
  FileOpen $0 "$INSTDIR\Sift Launcher.exe" a
  ${If} ${Errors}
    MessageBox MB_ICONSTOP|MB_OK "${PRODUCT} is running. Close it and run this uninstaller again."
    Quit
  ${EndIf}
  FileClose $0
  not_running:

  Delete "$SMPROGRAMS\${PRODUCT}\${PRODUCT}.lnk"
  Delete "$SMPROGRAMS\${PRODUCT}\Uninstall ${PRODUCT}.lnk"
  RMDir "$SMPROGRAMS\${PRODUCT}"
  Delete "$DESKTOP\${PRODUCT}.lnk"

  Delete "$INSTDIR\Sift Launcher.exe"
  Delete "$INSTDIR\sift.ico"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.md"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "${APP_PATH_KEY}"
  DeleteRegKey HKCU "${APP_KEY}"

  ; The data folder holds instances and worlds, so it is never removed silently:
  ; leaving it is the default answer even for /S. (An empty data folder is not
  ; worth a prompt — it holds nothing to lose.)
  IfFileExists "${DATA_DIR}\*.*" 0 done
  MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 \
    "Also delete ${PRODUCT}'s saved instances, worlds and settings?$\r$\n$\r$\nThis removes ${DATA_DIR} and cannot be undone." \
    /SD IDNO IDYES delete_data IDNO done
  delete_data:
    RMDir /r "${DATA_DIR}"
  done:
SectionEnd
