# message-acked-before-processed
langs: any

When: Queue or stream consumer added or changed, or its ack, offset-commit or auto-commit settings changed.
Ask: Is the message acknowledged before its effect commits? A crash in between then loses it: Celery acks a task before running it unless `acks_late=True`; Kafka `enable.auto.commit` commits offsets on a timer whatever processing did; a handler that catches the error and returns acks a failure as done. With the ack after the effect, is the handler idempotent for the redelivery? Does a message that always fails retry forever and block its partition or queue, or reach a dead-letter queue after a capped number of attempts?
Not when: losing a message is acceptable by contract (metrics, cache warm-up).
Report as: `<consumer>` acks `<message>` before `<effect>` commits: a crash or error in between drops it silently `<, and a poison message blocks the queue>`; fix: ack after the effect commits with an idempotent handler, and route repeated failures to a dead-letter queue.
