import unittest

from check import outcome_matches


class Outcomes(unittest.TestCase):
    def test_completed_check_requires_success_exit_and_completion(self):
        completed = "Model checking completed. No error has been found."
        self.assertTrue(outcome_matches(0, completed, "PASS"))
        self.assertFalse(outcome_matches(1, completed, "PASS"))
        self.assertFalse(outcome_matches(0, "Starting...", "PASS"))

    def test_mutant_requires_the_expected_violation_and_exit_code(self):
        trace = "Error: Invariant NoFork is violated.\nThe behavior up to this point is:"
        self.assertTrue(outcome_matches(12, trace, "NoFork"))
        self.assertFalse(outcome_matches(0, trace, "NoFork"))
        self.assertFalse(outcome_matches(1, trace, "NoFork"))
        self.assertFalse(outcome_matches(12, trace, "TypeOK"))
        self.assertFalse(outcome_matches(12, "Error: Parsing failed.", "NoFork"))
        self.assertFalse(outcome_matches(12, trace + "\nInvariant TypeOK is violated.", "NoFork"))


if __name__ == "__main__":
    unittest.main()
