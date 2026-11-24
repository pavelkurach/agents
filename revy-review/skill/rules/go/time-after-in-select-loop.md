# time-after-in-select-loop
langs: go

When: `time.After` or `time.NewTimer` called inside a `for`/`select` loop.
Ask: Is the limit meant for the whole loop or for each idle wait? A fresh `time.After` per iteration restarts the clock, so an overall deadline never fires while events keep arriving. Below Go 1.23 in the main module's `go.mod`, each pending timer stays allocated until it fires, so a hot loop grows memory; 1.23+ collects unreferenced timers.
Report as: `time.After` in the loop at `<site>` `<restarts the deadline / allocates a timer>` every iteration: `<the loop never times out under steady traffic / memory grows>`; fix: one timer or `context.WithTimeout` before the loop, or `Reset` one timer for an idle timeout.
