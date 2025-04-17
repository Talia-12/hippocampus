- Add back in more commenting on tests.
- Fix issues in common.rs where functions are claiming that they aren't being used when they clearly are.
- Add flag to control database path.
- Add flag to control logging level.
- ensure that the review handling to update card review times doesn't touch the database, so that I can reuse the same code
  to display to the user how long each button will defer for.
- Add a way to delete items
