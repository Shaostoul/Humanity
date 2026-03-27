# Auto-Updater Testing Checklist

Before every release, verify:

1. [ ] Current version shows correctly in Settings > Updates
2. [ ] "Check Now" detects the new version
3. [ ] Download progress bar works
4. [ ] Downloaded file is > 1MB (shown in F12 debug)
5. [ ] "Restart to Apply" message appears
6. [ ] After restart, title bar shows new version
7. [ ] Old .exe.old file exists next to the exe
8. [ ] F12 debug shows successful swap paths

Common failures:
- File size 0: download interrupted or wrong URL
- Same version after restart: rename failed (check F12 logs)
- "Error" state: check F12 for specific error message
