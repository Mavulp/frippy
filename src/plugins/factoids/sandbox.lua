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

local sandbox_env = {
  print = send,
  println = sendln,
  eval = nil,
  sleep = nil,
  args = args,
  input = input,
  user = user,
  channel = channel,
  request = download,
  string = string,
  math = math,
  table = table,
  pairs = pairs,
  ipairs = ipairs,
  next = next,
  select = select,
  unpack = unpack,
  tostring = tostring,
  tonumber = tonumber,
  type = type,
  assert = assert,
  error = error,
  pcall = pcall,
  xpcall = xpcall,
  _VERSION = _VERSION
}

sandbox_env.os = {
  clock = os.clock,
  time = os.time,
  difftime = os.difftime
}

sandbox_env.string.rep = nil
sandbox_env.string.dump = nil
sandbox_env.math.randomseed = nil

-- Temporary evaluation function
function eval(code)
  local c, e = load(code, nil, nil, sandbox_env)
  if c then
    return c()
  else
    error(e)
  end
end

-- Only sleeps for 1 second at a time
-- This ensures that the timeout check can still run
function safesleep(dur)
    while dur > 1000 do
        dur = dur - 1000
        sleep(1000)
    end
    sleep(dur)
end

sandbox_env.eval = eval
sandbox_env.sleep = safesleep

-- Check if the factoid timed out
function checktime(event, line)
  if os.time() - time >= timeout then
    error("Timed out after " .. timeout .. " seconds", 0)
  else
    -- Limit the cpu usage of factoids
    sleep(1)
  end
end

local f, e = load(factoid, nil, nil, sandbox_env)

-- Add timeout hook
time = os.time()
timeout = 30
debug.sethook(checktime, "l")

if f then
  f()
else
  error(e)
end
