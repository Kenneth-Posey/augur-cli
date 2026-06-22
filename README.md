## Rust-Powered Agentic Workflow CLI

![readme screenshot](resources/readme-screenshot.png)

This is a lightweight to run, but feature heavy, agentic CLI inspired by other tools like [Github Copilot CLI](https://github.com/features/copilot/cli/) and [Claude Code](https://claude.com/product/claude-code).

Augur-CLI is written in [Rust](https://rust-lang.org/) and heavily tuned for building Rust applications. This project includes the CLI source code, instruction files, built in agentic conversation flow, and a guided feature implementation pipeline. 

Currently the work on Augur-CLI is done using Augur-CLI and Deepseek v4 Flash via [Openrouter](https://openrouter.ai/). The repo root contains an installation bash script for installing the compiled rust binary from source into your home directory in .augur-cli, complete with configuration files alongside easily accessible directories for holding logs and session files. 

While it is tuned towards developing Rust applications, the tuning is entirely contained within the agent and skill files in /.github/, so updates to the instruction files would enable projects in other major programming languages. The design tries to make careful use of Openrouter's automatic caching to reduce costs. In my experience, a 100 million token session has a cost of roughly **$4** using Deepseek v4 Flash, so it's significantly cheaper than equivalent output from any other frontier model. 

This is a weekend solo-project that I started early April 2026, so there's rough edges, but it's production-ready enough that I switched from using Github Copilot CLI to using exclusively Augur-CLI for development. I find Deepseek v4 Flash to be roughly comparable in quality to Claude Sonnet 4.6, at a fraction of the cost, especially after caching. 

The goal is feature parity with other major LLM CLI platforms, plus quality-of-life upgrades that I found to be useful for my workflows. As an example of QoL, this detects when you launch from inside a git repository, and creates a dedicated conversation session directory and logging directory in the home config directory, so you don't have cross-repository contamination of your context by default.

### Quick-Installation (Linux)

Linux users can run the online installer to download and install the latest binary with supporting config files.

```bash 
bash <(curl -sL https://raw.githubusercontent.com/Kenneth-Posey/augur-cli/main/online-installer.sh)
```

### Configuration

The priority for loading configuration including .github files and the user/application is local directory first, then user home .augur-cli, then hardcoded defaults. The user home configuration is seeded on first launch if it doesn't already exist, so you'll need to update your application.secret.yml with API keys if you're not using the github copilot cli sdk integration. 

### Disclaimer and OS Warning

This was developed on Ubuntu 24, and while native MacOS versions are on my to-do list, I want to be more feature complete before I dedicate to going cross-platform with testing. Augur-cli uses the fantastic [Ratatui](https://ratatui.rs/) terminal-UI library, and it should hopefully work out of the box, but I'm leaning heavily on the library's cross-platform support. 

For MacOS, it should work running from source using [the dev launcher](augur-cli/launch-dev.sh) or [local source installer](augur-cli/install.sh). You should make sure the configuration file paths are correct and add your SDK keys to the application.secrets.yaml file. Refer to the [Install documentation](augur-cli/docs/INSTALL.md) for details. 

For Windows, good luck for now. I abandoned Windows as an operating system last year when Win10 went out of support and I have been using exclusively MacOS (work computer) and Ubuntu (personal computer) since then. I'm perfectly happy to help resolve issues with running on the windows operating system but for now due to time limitiations I can't set up a windows dev environment or proactively solve problems. 

### Features

* Included modular agents, skills, instruction files and prompts
* Agentic workflow loops when using Openrouter or Github Copilot CLI SDK
* Parallel background tasks with a panel for viewing live task output, separated by task
* Easy configuration of program settings, LLM providers and models with yml config files in user-home /.augur-cli
* Live LLM provider switching with **/switch** and model selection with **/model**
* Automatic conversation compaction, configurable per LLM model
* Manual conversation compaction with **/compact**
* Session saving in json files and resuming by the startup menu
* New sessions on demand with **/new-session**
* Detection of git repositories to self-organize conversation sessions and logs in the config home
* Rust LSP server for better development support
* Built-in tools for granular file modifications to reduce output-token use
* Flexible terminal-UI interface

### Upcoming features
#### Currently partially implemented, sorted by priority

* BETA: Steering of agentic workflows using the conversation model (currently works but needs some UI polish)
* BETA: Text file attachments with @file_path
* BETA: Built-in orchestrator pipeline for BDD/TDD development of major features
* BETA: Side-load conversations for asking questions outside the main context
* BETA: Deterministic standalone quality scanners for enforcing quality standards

### Future development
#### Currently not implemented, sorted by priority

* FUTURE: Free-only mode to support running only against configured *free* LLM providers and better support throttled requests
* FUTURE: Settings/config modification in-application
* FUTURE: Headless mode for server deployment (depends on docker containerization)
* FUTURE: Integration with the [VSCode Agents Window](https://code.visualstudio.com/docs/agents/agents-window) (depends on headless mode probably)
* FUTURE: Native MacOS builds and installer