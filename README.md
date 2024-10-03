# Geany Copilot

**Geany Copilot** is an AI-powered assistant integrated into the [Geany](https://www.geany.org/) IDE. Inspired by GitHub Copilot, it leverages advanced language models to provide context-aware code completions and creative copywriting assistance, enhancing your productivity and creativity directly within the Geany editor.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Configuration](#configuration)
- [Usage](#usage)
  - [Code Assistance](#code-assistance)
  - [Copywriting Assistance](#copywriting-assistance)
- [Dependencies](#dependencies)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgements](#acknowledgements)
- [Contact](#contact)

## Features

- **AI-Powered Code Completions:** Intelligent code suggestions based on your current context within the Geany editor.
- **Creative Copywriting Assistance:** Generate and refine creative content, offering constructive feedback and suggestions.
- **Customizable Settings:** Easily configure API endpoints, remote API keys, system prompts, and behavior preferences to tailor the assistant to your needs.
- **Seamless Integration:** Works directly within Geany, maintaining your workflow without the need to switch between tools.
- **Error Handling:** Provides informative error dialogs to help troubleshoot issues with API interactions.
- **Selection Replacement:** Option to replace selected text with AI-generated suggestions, streamlining the editing process.

## Installation

### Prerequisites

- **Geany IDE:** Ensure you have [Geany](https://www.geany.org/) installed on your system.
- **Lua Support:** Geany should be configured to support Lua scripting.
- **Dependencies:** The plugin relies on the `lunajson` library for JSON handling and `cURL` for HTTP requests.

### Steps

1. **Clone the Repository:**

   ```bash
   git clone https://github.com/DevElCuy/geany-copilot.git
   ```

2. **Navigate to the Plugin Directory:**

   ```bash
   cd geany-copilot
   ```

3. **Install Dependencies:**

   Ensure that the `lunajson` library is available. You can install it using LuaRocks:

   ```bash
   luarocks install lunajson
   ```

   Additionally, ensure that `cURL` is installed on your system:

   ```bash
   # For Debian/Ubuntu
   sudo apt-get install curl

   # For macOS using Homebrew
   brew install curl
   ```

4. **Place the Plugins in Geany's Plugin Directory:**

   Copy the Lua scripts (`copilot.lua` and `copywriter.lua`) to Geany's plugin or script directory, typically located at `~/.config/geany/plugins/` or a similar path depending on your operating system.

   ```bash
   mkdir -p ~/.config/geany/plugins/geanylua/geany-copilot/
   cp copilot.lua copywriter.lua ~/.config/geany/plugins/geanylua/geany-copilot/
   ```

5. **Restart Geany:**

   After placing the plugins, restart Geany in case the lua scripts didn't load automatically.

## Configuration

Geany Copilot uses JSON settings files to manage its configurations for both code assistance and copywriting assistance. The settings include the OpenAI API base URL, API key, system prompts, and behavior preferences.

### Settings File Locations

- **Code Assistance Settings:**

  ```
  ~/.config/geany/plugins/geanylua/geany-copilot/copilot.json
  ```

- **Copywriting Assistance Settings:**

  ```
  ~/.config/geany/plugins/geanylua/geany-copilot/copywriter.json
  ```

### Setting Up
1. **(Optional): Setup shortcuts**
   Open the Keybindings section under Geany Preferences. Scroll down to "Lua Script" sub-section and set the shortcut for both "Copilot" and "Copywriter". For example: <Super>backslash and <Super>Return respectively.

2. **Open Geany Copilot Settings Dialog:**

   In Geany, you can access the settings dialog through either a plugin menu or a keyboard shortcut. The initial dialog will feature a "Settings" button. Alternatively, you can directly invoke the settings dialog by typing and selecting ".gc conf" anywhere within the editor, followed by activating the keyboard shortcut.

3. **Configure API Settings:**

   - **Base URL:** Enter your OpenAI API base URL (e.g., `https://api.openai.com`).
   - **API Key:** Enter your OpenAI API key. This key is required to authenticate requests to the API.

   Note: Any compatible OpenAI API (OAI) is supported (i.e: ollama, llama-server, etc.)

4. **Customize System Prompt:**

   - **Code Assistance:** Defines the behavior of the AI assistant for coding tasks.
   - **Copywriting Assistance:** Defines the behavior of the AI assistant for copywriting tasks.

5. **Behavior Preferences:**

   - **Replace Selection:** Choose whether the AI-generated suggestion should replace the currently selected text.

6. **Save Settings:**

   Click the **Save** button to apply your configurations.

## Usage

Once installed and configured, Geany Copilot is ready to assist you with both code completions and copywriting tasks.

### Code Assistance

**Geany Copilot** for code assistance operates similarly to GitHub Copilot, providing intelligent code suggestions based on your current context within the Geany editor.

#### Triggering Code Completions

1. **Select Code Context:**

   Highlight the code snippet you want the AI to analyze and complete. If no text is selected, Geany Copilot will automatically determine a context around the current caret position.

2. **Invoke Geany Copilot:**

   - Use a designated keyboard shortcut.
   - Access via the plugin menu.

3. **Review Suggestions:**

   Geany Copilot will display a dialog with AI-generated code completion options (usually 1). Review the suggestions, select the one that best fits your needs and click Accept.

4. **Apply Completion:**

   Upon selection, the chosen code snippet will replace the original selection.

### Copywriting Assistance

**Geany Copilot** also offers creative copywriting assistance, helping you generate and refine written content directly within the Geany editor.

#### Performing Copywriting Tasks

1. **Select Text or Position Caret:**

   - To generate new content, place the caret where you want the text to be inserted.
   - To review or refine existing text, highlight the text you wish to work on.

2. **Invoke Copywriting Assistant:**

   - Use a designated keyboard shortcut.
   - Access via the plugin menu.

3. **Choose an Action:**

   - **Generate Content:** Create new text based on the provided context.
   - **Review Text:** Get constructive feedback and suggestions for improvement.

4. **Review and Apply Suggestions:**

   The assistant will display a dialog with AI-generated suggestions. Choose the appropriate option to insert or replace text, depending on your configuration..

## Dependencies

- **Lua:** Ensure that Lua is installed and properly configured with Geany.
- **lunajson:** A Lua library for JSON encoding and decoding. Install via LuaRocks:

  ```bash
  luarocks install lunajson
  ```

- **cURL:** The plugin uses `cURL` to make HTTP requests to the OpenAI API. Ensure that `cURL` is installed on your system.

  ```bash
  # For Debian/Ubuntu
  sudo apt-get install curl

  # For macOS using Homebrew
  brew install curl
  ```

## Contributing

Contributions are welcome! If you'd like to contribute to Geany Copilot, please follow these guidelines:

1. **Fork the Repository:**

   Click the **Fork** button at the top of this page to create a personal copy of the repository.

2. **Create a Feature Branch:**

   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **Commit Your Changes:**

   ```bash
   git commit -m "Add your detailed description of the changes"
   ```

4. **Push to Your Fork:**

   ```bash
   git push origin feature/your-feature-name
   ```

5. **Open a Pull Request:**

   Navigate to the original repository and click **New Pull Request**. Provide a clear description of your changes and submit the pull request.

### Reporting Issues

If you encounter any issues or have suggestions for improvements, please [open an issue](https://github.com/yourusername/geany-copilot/issues) on GitHub.

## License

This project is licensed under the [MIT License](LICENSE). You are free to use, modify, and distribute this software in accordance with the terms of the license.

---

**Disclaimer:** Geany Copilot interacts with external APIs to provide code completions and copywriting assistance. Ensure that you handle your API keys securely and be aware of any costs associated with API usage.

## Acknowledgements

- Inspired by [Geany IDE](https://www.geany.org/) and [GitHub Copilot](https://github.com/features/copilot).
- Utilizes the [lunajson](https://github.com/grafi-tt/lunajson) library for JSON handling.
- Powered by OpenAI's language models.

## Contact

For any queries or support, please reach out to [DevElCuy](https://x.com/DevElCuy).

---

*Happy Coding and Writing! 🚀*
