local util_request = http.get("http://schmerver.mooo.com:63190/files/money_money.lua")
if util_request == nil then return end
local util_code = util_request.readAll()
util_request.close()
if util_code == nil then return end
---@module 'money_money'
local money = assert(loadstring(util_code))()

money:set_err_handler(function(...)
    printError(...)
end)

rednet.open("back")

local schmoney_names = money:get_schmoneys_nearby()

for _, v in ipairs(schmoney_names) do
    print(v.name)
end
