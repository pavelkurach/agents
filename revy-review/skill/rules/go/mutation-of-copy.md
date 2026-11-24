# mutation-of-copy
langs: go

When: Field set on a range value variable or on a struct copied out of a slice or map element, or a value-receiver method that sets a field.
Ask: Is the modified value a copy? `for _, v := range items { v.Done = true }` changes nothing, and `&v` (a fresh variable per iteration in Go 1.22+) is still not the element; `it := m[k]; it.n++` needs a write back. A copy's pointer, map and slice fields still alias the original, so updates land half-applied.
Report as: `<update>` at `<site>` modifies a copy of `<element>`: the change is lost and `<consequence>`; fix: index (`items[i].Done = true`), write back (`m[k] = it`), a slice of pointers, or a pointer receiver.
