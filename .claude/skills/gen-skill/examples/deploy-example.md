# Example: Deploy Skill

A user-only task skill with side effects.

## SKILL.md

```yaml
---
name: deploy
description: Deploy the application to production. Use only when explicitly asked to deploy.
disable-model-invocation: true
argument-hint: [environment]
allowed-tools: Bash(git *), Bash(npm *), Bash(docker *), Read, Grep
---

# Deploy to $ARGUMENTS

Default environment: staging. Pass `production` for prod deploy.

## Pre-flight checks

1. Ensure all tests pass: `npm test`
2. Ensure no uncommitted changes: `git status`
3. Confirm the target branch is up to date: `git pull`

## Deploy steps

1. Build the application: `npm run build`
2. Run smoke tests: `npm run test:smoke`
3. Deploy to $ARGUMENTS: `npm run deploy:$ARGUMENTS`
4. Verify deployment health check

## Rollback

If any step fails, report the failure and suggest rollback steps. Do NOT auto-rollback without user confirmation.
```

## Why this works

- `disable-model-invocation: true` — prevents accidental deploys
- `allowed-tools` — restricts to only necessary commands
- Clear rollback instructions with user confirmation requirement
