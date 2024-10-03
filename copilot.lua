local _L = {}
local json = require("lunajson")

local context_length = 200
local DEFAULT = {
  ["oai-base-url"] = "http://localhost:10101",
  ["oai-api-key"] = "-",
  ["system-prompt"] = [[You are Geany Copilot, an AI programming assistant.
Analyze and complete the given CODE SNIPPET, and provide the resulting code.
Preserve identation. No explanations, or markdown formatting.]],
}

do
  -- The path to the settings file
  local filepath = geany.appinfo().scriptdir .. geany.dirsep .. "geany-copilot" .. geany.dirsep .. "copilot.json"

  -- Loads settings from a file
  -- @return table The parsed settings
  local function loadSettings()
    local parsed = {}
    local fh = io.open(filepath, "r")
    if fh then
      local content = fh:read("*a")
      parsed = json.decode(content) or parsed
      fh:close()
    end
    return parsed
  end

  -- Saves the given settings to the specified filepath.
  -- @param new The settings to be saved.
  local function saveSettings(new)
    local fh = io.open(filepath, "w")
    if fh then
      local out = json.encode(new)
      fh:write(out)
      fh:close()
    else
      geany.message("ERROR: Unable to save settings.")
    end
  end

  local settings = loadSettings()

  -- Sets a setting value in the settings table and saves it to the settings file.
  -- @param input Can be a key (string) or a callback (function).
  -- @return The current value of the setting or the default value if the key does not exist.
  function _L.setting(input)
    local type_ = type(input)
    if "function" == type_ then -- input is a callback
      input(settings)
      saveSettings(settings)
    elseif "string" == type_ then -- input is a key
      if nil == settings[input] then
        return DEFAULT[input]
      else
        return settings[input]
      end
    end
  end
end

-- Returns the current context (text) in the Geany editor.
-- If no text is selected, it calculates a default context around the current caret position.
function _L.getContext()
  local context = geany.selection()
  if context == "" then
    local pos = geany.caret()
    local start = pos - math.ceil(context_length/2)
    if start < 0 then
      start = 0
    end
    end_ = start + context_length

    geany.select(start, end_)
    context = geany.selection()
    geany.caret(pos)
  end

  return context
end

-- Returns a string containing information about the programming language.
function _L.getFileInfo()
  local info = geany.fileinfo()
  return ("Programming language: %s."):format(info.desc)
end

function _L.pushPrompt(prompt, callback)
  local filepath = os.tmpname()  -- Generate the path and name to a unique temporary file
  local fh = io.open(filepath, "w")
  local fileinfo = _L.getFileInfo()
  local query = {
    model = "gpt-4o-mini-2024-07-18",
    temperature = 0,
    max_tokens = 150,
    messages = {
      {
        role = "system",
        content = _L.setting("system-prompt")
      },
      {
        role = "user",
        content = fileinfo .. "\n\nCODE SNIPPET:\n" .. prompt .. "\n\n"
      },
    },
  }
  local out = json.encode(query)
  fh:write(out)
  fh:close()
  callback(filepath)
  os.remove(filepath) -- Remove the temporary file
end

function _L.llmInvoke(prompt)
  local response
  local parsed = {}
  local base_url = _L.setting("oai-base-url")
  local api_key = _L.setting("oai-api-key")
  if base_url and api_key then
    _L.pushPrompt(prompt, function (prompt_filepath)
      local command = ([[curl -s --url %s/v1/chat/completions --header "Content-Type: application/json" --header "Authorization: Bearer %s" --data-binary @%s]]):format(base_url, api_key, prompt_filepath)
      local handle = io.popen(command)
      response = handle:read("*a") -- capture response
      handle:close()
    end)
    if response and response ~= "" then
      parsed = json.decode(response) or parsed
    end
  end
  return parsed
end

function _L.settingsDialog()
  local dlg = dialog.new("Settings - Geany Copilot", { "_Save", "_Cancel"})

  dlg:heading("OpenAI API")
  dlg:text("oai-base-url", _L.setting("oai-base-url"), "Base URL:")
  dlg:password("oai-api-key", _L.setting("oai-api-key"), "API key:")
  dlg:heading("System prompt")
  dlg:textarea("system-prompt", _L.setting("system-prompt"))

  local button, results = dlg:run()

  if button == 1 then
    _L.setting(function (t)
      t["oai-base-url"] = results["oai-base-url"]
      t["oai-api-key"] = results["oai-api-key"]
      t["system-prompt"] = results["system-prompt"]
    end)
  end
end

function _L.errorDialog(msg)
  local dlg = dialog.new("Error - Geany Copilot", { "_Accept", "_Settings"})
  dlg:heading(msg)

  local button = dlg:run()

  if button == 2 then
    _L.settingsDialog()
  end
end

function _L.copilotDialog(context, choices)
  local dlg = dialog.new("Geany Copilot", {"_Accept", "_Reject", "_Settings"})

  dlg:label(("Original:\n\n%s\n"):format(context))

  dlg:label("Recommendations:")
  for k, v in pairs(choices or {}) do
    if v and v.message and v.message.content then
      dlg:radio("choice", k, v.message.content)
    end
  end

  -- Show the dialog
  local button, results = dlg:run()

  if button == 3 then
    _L.settingsDialog()
  end

  if button == 1 and results then
    geany.selection(choices[tonumber(results.choice)].message.content)
  end
end

function main ()
  local context = _L.getContext()
  if ".gc conf" == context then
    _L.settingsDialog()
    return
  end
  local result = _L.llmInvoke(context)
  if result.choices then
    _L.copilotDialog(context, result.choices)
  elseif result.error then
    _L.errorDialog(result.error.message)
  else
    _L.errorDialog("Unknown error.")
  end
end

main()