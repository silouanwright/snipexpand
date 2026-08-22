vim.bo.filetype = "yaml"
vim.b.completion = false
vim.g.minipairs_disable = true

vim.opt_local.spell = false
vim.opt_local.wrap = false
vim.opt_local.conceallevel = 0
vim.opt_local.numberwidth = 3
vim.opt_local.signcolumn = "no"
vim.opt_local.statuscolumn = "%=%3l "
vim.opt_local.formatoptions:remove({ "c", "r", "o" })

vim.diagnostic.enable(false, { bufnr = 0 })
vim.diagnostic.reset(nil, 0)

local has_which_key, which_key_config = pcall(require, "which-key.config")
if has_which_key then
  if not vim.tbl_contains(which_key_config.disable.ft, "yaml") then
    table.insert(which_key_config.disable.ft, "yaml")
  end
  require("which-key.state").stop()
  require("which-key.buf").clear({ buf = 0 })
end

vim.keymap.set("n", "<F11>", function()
  for row, line in ipairs(vim.api.nvim_buf_get_lines(0, 0, -1, false)) do
    local start_column = line:find('"ls"', 1, true)
    if start_column then
      vim.api.nvim_win_set_cursor(0, { row, start_column + 2 })
      vim.cmd.startinsert()
      return
    end
  end
end, { buffer = true, silent = true })

vim.api.nvim_win_set_cursor(0, { 1, 0 })
