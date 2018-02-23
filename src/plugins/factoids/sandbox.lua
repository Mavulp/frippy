function send(text)
  local text = tostring(text)
  local len = #output
  if len < 1 then
    output = { text }
  else
    output[len] = output[len] .. text
  end
end

function sendln(text)
  send(text)
  table.insert(output, "")
end

local env = { print = send,
              println = sendln,
              args = args,
              input = input,
              user = user,
              channel = channel,
              request = download,
              pairs = pairs,
              table = table,
              string = string,
              tostring = tostring,
              tonumber = tonumber,
              math = math }

local f, e = load(factoid, nil, nil, env)

-- Check if the factoid timed out
function checktime(event, line)
    if os.time() - time >= timeout then
        error("Timed out after " .. timeout .. " seconds", 0)
    else
        -- Limit the cpu usage of factoids
        sleep(1)
    end
end

-- Add timeout hook
time = os.time()
timeout = 30
debug.sethook(checktime, "l")

if f then
  f()
else
  error(e)
end
