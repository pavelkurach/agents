# blocking-call-in-async
langs: any

When: Sync I/O, a sleep, a sync client or a long CPU loop added inside async code running on an event loop, directly or through a sync helper it calls.
Ask: Follow the call chain from the `async` function: does it reach a blocking call — `requests`, `boto3`, a sync DB driver, `time.sleep`, `std::fs`, `std::thread::sleep`, `fs.*Sync`, a sync SDK? How long does CPU work run on real input sizes? Is it offloaded (`asyncio.to_thread`, `run_in_executor`, `tokio::task::spawn_blocking`)?
Not when: a one-shot script with no other tasks on the loop.
Report as: `<call>` blocks the loop in `<fn>`: every task on that loop stalls for its duration (latency spikes, missed timeouts and heartbeats); fix: an async client, or offload to a thread pool.
