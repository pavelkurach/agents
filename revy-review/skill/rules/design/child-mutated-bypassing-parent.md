# child-mutated-bypassing-parent
langs: any

When: Child record or collection changed directly (`order.items.append`, `line.qty = …`) where a parent keeps a rule or derived value over its children.
Ask: Does the parent maintain a total, count, limit, uniqueness or status over its children — read its methods and where those fields are set? Does this path update or check it? Does the parent hand out its collection mutably, so any caller can do the same?
Not when: the parent keeps no rule over the children.
Report as: `<path>` changes `<child>` behind `<parent>`'s back: its `<total / limit / status>` is now wrong; fix: change children only through a `<parent>` method that enforces the rule, and expose them read-only.
