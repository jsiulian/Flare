# MR Checklist

This is a small list of things you should check before doing an MR.
This list is not strict, e.g. I also take code that is not well-formatted with `cargo fmt`, at least to some degree.

- Make sure the code passes some basic checks and builds and does not give warnings or crash while running.
- Make sure the code is properly formatted (`cargo fmt`, in many cases already done by your IDE).
- Optional but encouraged: Check for new warnings in `cargo clippy`.
- Does the code still include TODOs or commented out code? Those are fine as long as it is a draft, but the reviewer will likely ask about those when it is not a draft anymore.
- Did you copy parts of the code from some other project? Ensure this is compliant with the license and also add a comment to the file code it originated from.
