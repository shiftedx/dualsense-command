# Out-Of-Scope Decisions

This directory stores persistent records for enhancement requests that are
closed as `wontfix`.

Use one Markdown file per rejected concept, not one file per issue. During
triage, check this directory before re-litigating a similar request.

## File Format

```markdown
# Concept Name

This feature is out of scope for DSCC.

## Why this is out of scope

Explain the durable product or technical reason. Avoid temporary reasons such
as "not enough time right now."

## Prior requests

- #123 - Short issue title
```

If the maintainer later reconsiders the decision, update or remove the concept
file and let the new issue proceed through normal triage.
