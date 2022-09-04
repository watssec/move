## Higher-order mutation reports

This file contains an initial sample of some higher-order mutation result from mutating move.

The next step is to merge 2-step mutation with the original combination (two isolated points) in order to get a time/mutation type distribution for the evaluation part.

## Report 1

```
if (i > size) {
    return false
};
```

[Original Code](https://github.com/Kindhearted57/diem/blob/dd8a3fc6570dcf3d8935bed18d0c54fedc69e042/diem-move/diem-framework/DPN/sources/DiemSystem.move#L513)

Report Content:

```
Mutation Points [Some(Loc { file_hash: "4c018efff1bc6a423543d22e9509c332ecce4fd1736440cba28355f7245322b0", start: 24369, end: 24378 }), Some(Loc { file_hash: "4c018efff1bc6a423543d22e9509c332ecce4fd1736440cba28355f7245322b0", start: 24369, end: 24378 })]
Mutation Types ["BinOp Ge to Le", "BinOp Ge to Gt"]

warning[W13001]: Prover passed after arithmetic operator is mutated
    ┌─ ./sources/DiemSystem.move:512:13
    │
512 │         if (i >= size) {
    │             ^^^^^^^^^ prover passed after mutation
```

The first mutation mutate` >=` to `<=`, still result in error. The second mutation mutate `>=` to `>` and lead to mutation passing the prover.

# Report 2

[Original Code](https://github.com/Kindhearted57/diem/blob/dd8a3fc6570dcf3d8935bed18d0c54fedc69e042/diem-move/diem-framework/DPN/sources/AccountLimits.move#L476)

```
if (outflow_ok) {
    sending.window_outflow = sending.window_outflow + amount;
    sending.tracked_balance = if (amount >= sending.tracked_balance) 0
                               else sending.tracked_balance - amount;
};
```

Firstly mutate `>=` to `<`, error, mutate `>=` to `>` prover pass.

# Report 3 

```
        while ({
            spec {
                invariant i <= size;
                invariant forall j in 0..i: validators[j].addr != addr;
            };
            (i < size)
        })
        {
            let validator_info_ref = Vector::borrow(validators, i);
            if (validator_info_ref.addr == addr) {
                spec {
                    assert validators[i].addr == addr;
                };
                return Option::some(i)
            };
            i = i + 1;
        };
```
[Original Code](https://github.com/Kindhearted57/diem/blob/dd8a3fc6570dcf3d8935bed18d0c54fedc69e042/diem-move/diem-framework/DPN/sources/DiemSystem.move#L482)

Mutate `i = i + 1` => `i = i - 1` => error, mutate `+` to `/` `*`, pass