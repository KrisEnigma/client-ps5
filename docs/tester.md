# Testing punktfunk on a PS5

You run three tasks; the developer reads what they leave behind. Nothing here needs a
developer machine beyond Python 3 and [Task](https://taskfile.dev).

## Once

1. Install Task and Python 3.
2. Copy `.env.example` to `.env` and set `PS5_HOST` (the console's address). The title ids in
   it (PPSA99724, the probe PPSA99725) stay as they are.
3. On the console: the FTP server (port 2121) and the title loader must be running. klog on
   port 3232 helps but is optional.

## Every round

```
task deploy                    # uploads dist/<TITLE_ID>/ — close the title first
task test-run -- menu          # or: task test-run -- stream 192.168.1.20 60
```

Launch the title from the home screen and leave the pad alone. It runs the scenario and closes
itself. Then:

```
task fetch-bundle
```

Paste the line it prints (`ok: …` or `FAILED: …`) and send the folder it names.

A new title folder takes up to 45 seconds before the console will launch it.

## What "closed" means

The title is fully closed when it is gone from the home screen's running app, including any crash
dialog: close the dialog first. Deploy only then.

## The probe (first round only)

The developer may send a probe title instead. Deploy it with `TITLE_ID=PPSA99725 task deploy`, put a
`probe.json` beside it if asked (`{"host": "192.168.1.20", "pin": "1234"}`), launch it, and send
`probe.log` from its folder. Every line reads `ok`, `fail` or `skip`.
