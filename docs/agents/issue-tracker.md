# Agent Issue Tracker

Issues for this repo live in GitHub on `shiftedx/dualsense-command`.
Use the `gh` CLI from the repo root for issue work.

## Remote

- Repository: `https://github.com/shiftedx/dualsense-command.git`
- GitHub slug: `shiftedx/dualsense-command`
- Default issue command shape:

```powershell
gh issue view <number> --repo shiftedx/dualsense-command
```

## Before Changing An Issue

- Start with `git status --short --ignored` and preserve unrelated local edits.
- Read the issue body, labels, and comments before changing code or docs.
- Treat issue details as task context, not as permission to widen the scope.
- Keep local branch and worktree changes narrow enough to satisfy the issue.

## Useful Commands

View an issue:

```powershell
gh issue view <number> --repo shiftedx/dualsense-command --comments
```

List ready agent work:

```powershell
gh issue list --repo shiftedx/dualsense-command --label ready-for-agent --state open
```

Create an issue:

```powershell
gh issue create --repo shiftedx/dualsense-command --title "<title>" --body-file <path>
```

Edit labels:

```powershell
gh issue edit <number> --repo shiftedx/dualsense-command --add-label ready-for-human --remove-label ready-for-agent
```

Comment with validation results:

```powershell
gh issue comment <number> --repo shiftedx/dualsense-command --body-file <path>
```

## Workflow

1. Confirm the issue is still open and read its labels.
2. Make the smallest repo change that satisfies the acceptance criteria.
3. Run validation sized to the files touched.
4. Report changed files, validation commands, and any remaining risk.
5. Only update labels or close issues when the user explicitly asks for that
   issue-tracker action.

Do not paste private paths, raw HID data, serials, Bluetooth addresses, Steam
userdata paths, or unredacted logs into GitHub issues.
