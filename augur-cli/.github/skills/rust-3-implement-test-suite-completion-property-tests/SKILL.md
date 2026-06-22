---
name: rust-3-implement-test-suite-completion-property-tests
description: >
  Property-based testing patterns for Rust using proptest and arbitrary. Load
  when implementing invariant checks or fuzz-style coverage over domain types.
---

# Skill: Rust Test Suite Completion - Property-Based Testing Patterns

---

## Property-Based Testing with `proptest`

### Basic Property Test Structure

`proptest!` generates random inputs and checks the same property across them:

```rust
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_sort_preserves_length(
            mut vec in prop::collection::vec(0i32..100, 0..1000)
        ) {
            vec.sort();
            assert_eq!(vec.len(), 0); // Initial vec length
        }
    }
}
```

**Key Points**:
- `prop::collection::vec(...)` builds random vectors.
- First argument: element strategy (`0i32..100`).
- Second argument: length range (`0..1000`).
- `proptest` runs about 256 cases by default.
- On failure, it shrinks to a minimal counterexample.

---

### Arbitrary Implementation for Custom Types

Implement `Arbitrary` for custom types:

```rust
use proptest::prelude::*;

#[derive(Clone, Debug)]
pub struct User {
    name: String,
    age: u32,
    email: String,
}

impl Arbitrary for User {
    type Parameters = ();
    type Strategy = impl Strategy<Tree = impl ValueTree<Value = Self>, Error = TestCaseError>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (
            "[a-z]+",           // Names: lowercase letters
            18u32..120,          // Age: 18 to 120
            "[a-z]+@[a-z]+\\.com" // Email: simple format
        )
            .prop_map(|(name, age, email)| User {
                name,
                age,
                email,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_user_is_valid_when_created(user in any::<User>()) {
            assert!(!user.name.is_empty());
            assert!(user.age >= 18);
            assert!(user.email.contains('@'));
        }
    }
}
```

**Key Points**:
- Implement `Arbitrary` with `arbitrary_with`.
- Use `prop_map` to turn generated primitives into your type.
- Combine strategies with tuples.
- Use regex patterns for string generation.

---

### Composing Strategies

Combine strategies for multi-value scenarios:

```rust
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_dict_operations(
            key in "[a-z]+",
            value in any::<i32>(),
            other_key in "[a-z]+"
        ) {
            let mut dict = HashMap::new();
            dict.insert(key.clone(), value);

            // Property: Inserted value should be retrievable
            assert_eq!(dict.get(&key), Some(&value));

            // Property: Other key should not be found
            if key != other_key {
                assert_eq!(dict.get(&other_key), None);
            }
        }
    }
}
```

**Key Points**:
- Each `proptest!` parameter is generated independently.
- Each parameter uses its own strategy.
- Generated values satisfy the declared constraints.
- This is useful for testing interactions between values.

---

### Encoding Mathematical Invariants

Use property tests to verify invariants:

```rust
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        // Invariant: Sorted array length equals input length
        #[test]
        fn test_sort_preserves_length(
            mut vec in prop::collection::vec(0i32..1000, 1..100)
        ) {
            let original_len = vec.len();
            vec.sort();
            prop_assert_eq!(vec.len(), original_len);
        }

        // Invariant: All elements remain after sort
        #[test]
        fn test_sort_preserves_elements(
            mut vec in prop::collection::vec(0i32..100, 1..100)
        ) {
            let original = vec.clone();
            vec.sort();
            
            for elem in original {
                prop_assert!(vec.contains(&elem));
            }
        }

        // Invariant: Sorted array is non-decreasing
        #[test]
        fn test_sorted_array_is_ordered(
            mut vec in prop::collection::vec(0i32..1000, 1..100)
        ) {
            vec.sort();
            
            for i in 0..vec.len() - 1 {
                prop_assert!(vec[i] <= vec[i + 1]);
            }
        }
    }
}
```

**Key Points**:
- Keep each invariant in its own property test.
- Use `prop_assert!` to report failures in generated cases.
- Separate invariants catch different aspects of behavior.
- Shrinking reveals the smallest failing case.

---

## Key Files

- `README.md` - overview and usage notes
