## Rust-Powered Agentic Workflow CLI

This is a lightweight to run, but feature heavy, agentic CLI inspired by other tools like [Github Copilot CLI](https://github.com/features/copilot/cli/) and [Claude Code](https://claude.com/product/claude-code).

Augur-CLI is written in [Rust](https://rust-lang.org/) and heavily tuned for building Rust applications. This project includes the CLI source code, instruction files, built in agentic conversation flow, and a guided feature implementation pipeline. 

Currently the work on Augur-CLI is done using Augur-CLI and Deepseek v4 Flash via [Openrouter](https://openrouter.ai/). The repo root contains an installation bash script for installing the compiled rust binary from source into your home directory, complete with configuration files alongside easily accessible directories for holding logs and session files. 

While it is tuned towards developing Rust applications, the tuning is entirely contained within the agent and skill files in /.github/, so updates to the instruction files would enable projects in other major programming languages. The design tries to make careful use of Openrouter's automatic caching to reduce costs, so a 100 million token session costs roughly $4. 

This is a weekend solo-project that I started the first week of April 2026, so there's rough edges, but it's production-ready enough that I switched from using Github Copilot CLI to using Augur-CLI for development. I find Deepseek v4 Flash to be roughly comparable in quality to Claude Sonnet 4.6, at a fraction of the cost, especially after caching. 

The goal is feature parity with other major CLI platforms, plus quality-of-life upgrades that I found to be useful for my workflows. 

Disclaimer, this was developed on Ubuntu 24, and while Windows and MacOS versions are on my to-do list, I want to be more feature complete before I go cross-platform. This uses the fantastic [Ratatui](https://ratatui.rs/) terminal-UI library, so the next intermediate step is docker containerization for better cross platform support, before true cross-platform installers. For now it still works great running from source. 

##Features

* Included modular agents, skills, instruction files and prompts
* Agentic workflow loops when using Openrouter or Github Copilot CLI SDK
* Parallel background tasks with a panel for viewing live task output, separated by task
* Easy configuration of program settings, LLM providers and models with yml config files in %userhome%
* Live LLM provider switching with **/switch** and model selection with **/model**
* Automatic conversation compaction, configurable per LLM model
* Manual conversation compaction with **/compact**
* Session saving in json files and resuming by the startup menu
* New sessions on demand with **/new-session**
* Detection of git repositories to self-organize sessions and logs in %userhome%
* Rust LSP server for better development support
* Built-in tools for granular file modifications to reduce output-token use
* Flexible terminal-UI interface
* BETA: Built-in orchestrator pipeline for BDD/TDD development of major features
* BETA: Text file attachments with @file_path
* BETA: Deterministic standalone quality scanners for enforcing quality standards
* BETA: Steering of agentic workflows using the conversation model
