# equal-values-not-canonical
langs: any

When: Constructor, parser or input path added for a value that is compared, hashed, used as a key, deduplicated or serialized.
Ask: Can two values the domain treats as equal — case, whitespace, unicode normalization form, unit, trailing zero, order of a set-like field — come out in different forms? Find every constructor and parse path: does each normalize the same way, and do equality, hash and serialized form agree?
Report as: `<type>` built via `<path>` keeps `<raw form>`, so equal values `<a>` and `<b>` compare or hash unequal: `<duplicates / missed lookups>`; fix: normalize in the one constructor every path goes through.
