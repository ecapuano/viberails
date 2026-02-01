# Viberails

**Enterprise-grade visibility and control for AI coding assistants**

Viberails gives your team complete oversight of AI coding tool activity across your organization. Know what your AI assistants are doing, catch risky operations before they happen, and maintain compliance—all without slowing down your developers.

## Why Viberails?

AI coding assistants like Claude Code, Cursor, and Copilot are transforming how developers work. But with great power comes great responsibility:

- **What tools are your AI assistants executing?** File operations, shell commands, API calls—do you know what's happening across your team?
- **Are sensitive files being accessed?** Configuration files, credentials, proprietary code—AI assistants can read and modify anything.
- **How do you maintain compliance?** Auditors want logs. Security teams want visibility. Viberails delivers both.

### The Value

| Without Viberails                | With Viberails                       |
| -------------------------------- | ------------------------------------ |
| No visibility into AI tool usage | Complete audit trail of every action |
| Risky operations go unnoticed    | Real-time authorization controls     |
| Compliance gaps                  | Full audit logs for security reviews |
| Each developer is an island      | Team-wide visibility and policies    |
| Hope nothing goes wrong          | Know exactly what's happening        |

## Features

- **Multi-Tool Support** - Works with the most popular AI coding assistants:
    - Claude Code
    - Cursor
    - Gemini CLI
    - OpenAI Codex CLI
    - OpenCode
    - Clawdbot/OpenClaw

- **Zero Friction Setup** - Install in under 2 minutes with automatic tool detection

- **Team Collaboration** - One person sets up the team, everyone else joins with a single command

- **Privacy Controls** - Choose what gets sent to the cloud:
    - Full audit (tool calls + prompts)
    - Tool authorization only (no prompts)
    - Fully local operation

- **Fail-Safe Design** - Configurable fail-open/fail-closed behavior ensures developers aren't blocked

## Quick Start

### 1. Create Your Team

**macOS/Linux:**

```bash
# Download and run
curl -sSL https://get.viberails.io | bash

# Initialize your team (opens browser for OAuth)
viberails init-team
```

**Windows (PowerShell):**

```powershell
# Download and run
irm https://get.viberails.io/install.ps1 | iex

# Initialize your team (opens browser for OAuth)
viberails init-team
```

You'll be prompted to select an OAuth provider (Google, Microsoft, or GitHub) and enter your team name. The browser will open for authentication.

To use an existing LimaCharlie organization:

```bash
viberails init-team --existing-org
```

### 2. Install Hooks

```bash
viberails install
```

Viberails automatically detects which AI tools you have installed and lets you choose which ones to hook:

```
? Select AI coding tools to install hooks for:
  [x] Claude Code [detected]
  [x] Cursor [detected]
  [ ] Gemini CLI [not found]
  [ ] OpenAI Codex CLI [not found]
```

### 3. Add Your Team

Share the team URL with your colleagues. They can join without OAuth:

```bash
# On each team member's machine
viberails join-team https://hooks.example.com/your-team-url
viberails install
```

That's it! Your team now has complete visibility into AI coding assistant activity.

## Configuration

### View Current Settings

```bash
viberails show-config
```

### Privacy Controls

Control what data is sent to your team's cloud:

```bash
# Keep prompts/chat local (only send tool authorizations)
viberails configure --audit-prompts false

# Approve all tools locally (no cloud authorization)
viberails configure --audit-tool-use false

# Re-enable full auditing
viberails configure --audit-prompts true --audit-tool-use true
```

### Fail-Open Behavior

By default, if the cloud is unreachable, tools are approved locally. To enforce strict mode:

```bash
viberails configure --fail-open false
```

## Commands Reference

| Command           | Description                         |
| ----------------- | ----------------------------------- |
| `init-team`       | Create a new team via OAuth         |
| `join-team <URL>` | Join an existing team               |
| `install`         | Install hooks for detected AI tools |
| `uninstall`       | Remove hooks from AI tools          |
| `list`            | Show installed hooks                |
| `configure`       | Modify settings                     |
| `show-config`     | Display current configuration       |
| `upgrade`         | Update to the latest version        |

## Architecture

### Backend: LimaCharlie

Viberails is powered by [LimaCharlie](https://limacharlie.io), a security infrastructure platform trusted by enterprises worldwide. This gives you:

- **Enterprise-grade reliability** - Built on battle-tested security infrastructure
- **Scalability** - From small teams to large organizations
- **Security** - Your data is handled with the same care as enterprise security telemetry
- **Extensibility** - Integrate with your existing security tools and workflows

### How It Works

```
┌─────────────────────────────────────────────────────────────┐
│ AI Coding Tool (Claude Code, Cursor, etc.)                  │
└────────────────────────┬────────────────────────────────────┘
                         │ Hook triggers on tool use
                         ▼
┌─────────────────────────────────────────────────────────────┐
│ Viberails Hook                                              │
│ • Captures tool call details                                │
│ • Sends to cloud for authorization                          │
│ • Returns allow/deny decision                               │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│ LimaCharlie Backend                                         │
│ • Stores audit logs                                         │
│ • Evaluates authorization policies                          │
│ • Provides team-wide visibility                             │
└─────────────────────────────────────────────────────────────┘
```

## Web Application (Coming Soon)

We're building a web dashboard that will provide:

- **Real-time Activity Feed** - Watch AI tool usage across your team as it happens
- **Policy Management** - Define rules for what tools can and cannot do
- **Analytics & Reporting** - Understand AI tool usage patterns
- **Alerts** - Get notified when sensitive operations occur
- **Compliance Reports** - Generate audit reports with one click

Stay tuned for updates!

## Building from Source

```bash
# Clone the repository
git clone https://github.com/refractionPOINT/viberails.git
cd viberails

# Build
cargo build --release

# Run
./target/release/viberails --help
```

## Requirements

- Windows, macOS, or Linux
- One or more supported AI coding tools installed
- Internet connection for team features (or use local-only mode)

## Contributing

We welcome contributions! Before submitting code:

```bash
cargo clippy -- -D warnings  # No warnings allowed
cargo test                   # All tests must pass
```

These checks are enforced by pre-commit hooks.

## Support

- **Issues**: [GitHub Issues](https://github.com/refractionPOINT/viberails/issues)
- **Documentation**: [docs.viberails.io](https://docs.viberails.io) (coming soon)
- **Community**: [Discord](https://discord.gg/viberails) (coming soon)

---

**Built with security in mind by [Refraction Point](https://refractionpoint.com), the team behind [LimaCharlie](https://limacharlie.io).**
