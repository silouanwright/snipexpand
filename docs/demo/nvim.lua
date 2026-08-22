vim.bo.filetype = "markdown"
vim.b.completion = false
vim.b.render_markdown = false
vim.g.minipairs_disable = true

vim.opt_local.spell = false
vim.opt_local.autoindent = false
vim.opt_local.smartindent = false
vim.opt_local.cindent = false
vim.opt_local.indentexpr = ""
vim.opt_local.wrap = false
vim.opt_local.conceallevel = 0
vim.opt_local.numberwidth = 3
vim.opt_local.signcolumn = "no"
vim.opt_local.statuscolumn = "%=%3l "
vim.opt_local.whichwrap:append("<,>,[,]")

vim.diagnostic.enable(false, { bufnr = 0 })
vim.diagnostic.reset(nil, 0)

local has_which_key, which_key_config = pcall(require, "which-key.config")
if has_which_key then
  if not vim.tbl_contains(which_key_config.disable.ft, "markdown") then
    table.insert(which_key_config.disable.ft, "markdown")
  end
  require("which-key.state").stop()
  require("which-key.buf").clear({ buf = 0 })
end

vim.keymap.set("i", "<F12>", function()
  local cursor_line = vim.api.nvim_win_get_cursor(0)[1]
  local window_height = vim.api.nvim_win_get_height(0)
  local top_line = math.max(1, cursor_line - math.floor(window_height / 2))
  vim.fn.winrestview({ topline = top_line })
end, { buffer = true, silent = true })

vim.api.nvim_buf_set_lines(0, -1, -1, false, { "", "" })
vim.api.nvim_win_set_cursor(0, { vim.api.nvim_buf_line_count(0), 0 })
vim.cmd.startinsert()
