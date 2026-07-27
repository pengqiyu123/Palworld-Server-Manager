Var PalworldBackupsPreserved

!macro NSIS_HOOK_POSTINSTALL
  StrCpy $PalworldBackupsPreserved "$INSTDIR.backups-preserved"
  IfFileExists "$PalworldBackupsPreserved\*.*" 0 backups_reinstall_done
  IfFileExists "$INSTDIR\backups\*.*" backups_reinstall_done 0
  Rename "$PalworldBackupsPreserved" "$INSTDIR\backups"
  IfErrors backups_reinstall_failed backups_reinstall_done

backups_reinstall_failed:
  IfSilent backups_reinstall_silent backups_reinstall_message

backups_reinstall_message:
  MessageBox MB_ICONEXCLAMATION|MB_OK "旧备份仍安全保存在 $PalworldBackupsPreserved，请手动合并到 $INSTDIR\backups。"
  Goto backups_reinstall_done

backups_reinstall_silent:
  DetailPrint "旧备份仍保存在 $PalworldBackupsPreserved"

backups_reinstall_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  StrCpy $PalworldBackupsPreserved "$INSTDIR.backups-preserved"
  IfFileExists "$INSTDIR\backups\*.*" 0 backups_preserve_done
  IfFileExists "$PalworldBackupsPreserved\*.*" 0 backups_preserve_move
  MessageBox MB_ICONSTOP|MB_OK "无法安全卸载：备份保护目录已存在。请先处理 $PalworldBackupsPreserved。"
  Abort

backups_preserve_move:
  Rename "$INSTDIR\backups" "$PalworldBackupsPreserved"
  IfErrors 0 backups_preserve_done
  MessageBox MB_ICONSTOP|MB_OK "无法移动备份目录。为避免丢失存档，卸载已取消。"
  Abort

backups_preserve_done:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DetailPrint "备份已保留在 $INSTDIR.backups-preserved"
!macroend
