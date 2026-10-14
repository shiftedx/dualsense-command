# Agent Triage Labels

Use GitHub labels on `shiftedx/dualsense-command` to describe both the issue
kind and its current handoff state.

## Category Labels

Apply at least one category label when the issue type is clear.

| Role | GitHub label | Use when |
| --- | --- | --- |
| Bug | `bug` | Existing behavior is broken, missing, unsafe, or inconsistent with documented behavior. |
| Enhancement | `enhancement` | The issue adds or improves behavior beyond the current contract. |
| Documentation | `documentation` | The issue creates, restores, or corrects docs, guidance, release notes, or support copy. |

## State Labels

Use one state label at a time. Replace the old state when the issue moves.

| Role | GitHub label | Use when |
| --- | --- | --- |
| Needs triage | `needs-triage` | The issue has not been classified or needs an owner to decide next steps. |
| Needs info | `needs-info` | Work is blocked on missing reproduction details, decisions, files, or credentials. |
| Ready for agent | `ready-for-agent` | The scope and acceptance criteria are clear enough for an agent to implement. |
| Ready for human | `ready-for-human` | The next step needs maintainer review, product judgment, hardware validation, or manual release action. |
| Won't fix | `wontfix` | The project intentionally declines the request or bug report. |

## Label Rules

- Keep category and state separate. For example, a docs bug can have
  `bug`, `documentation`, and `ready-for-agent`.
- Do not leave two state labels on the same issue unless the user explicitly
  asks for an intermediate cleanup.
- Move to `needs-info` when acceptance criteria cannot be tested from the repo
  and the missing detail is material.
- Move to `ready-for-human` when implementation is done but requires human
  verification, such as hardware testing, release signing, or product approval.
- Use `wontfix` only when the decision is explicit in the issue, PRD, or user
  instruction.

## Validation Notes

When reporting back on a triaged or completed issue, include:

- The issue number and current labels.
- The changed files or decision made.
- The validation command output summary.
- Any follow-up that should be tracked separately.
