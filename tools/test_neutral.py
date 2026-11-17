#!/usr/bin/env python3
"""Unit tests of tools/neutral.py on small fixtures (run by tools/check.sh quick):

    python3 -I tools/test_neutral.py
"""

import copy
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import neutral  # noqa: E402

MATCH = {
    "hash": "0x1",
    "seed": 1,
    "side": "normal",
    "result": "won",
    "winner_faction": "ww2:usa",
    "winner_base": 1,
    "length_s": 982.8,
    "he_share_pct": 25.9,
    "tanks_lost": [5, 3],
}
BASE = {
    "schema": 1,
    "jobs": 3,
    "kind": "skirmish",
    "matches": 2,
    "tick_p99_ms": {"all": 0.4},
    "wins": {"germany": 1, "usa": 1},
    "factions": {"germany": {"kills": 10}},
    "per_match": [MATCH, dict(MATCH, side="swapped", hash="0x2", winner_base=0)],
}


class Neutral(unittest.TestCase):
    def run_dirs(self, base, new, name="skirmish40.json"):
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            for d, doc in ((a, base), (b, new)):
                with open(os.path.join(d, name), "w") as f:
                    json.dump(doc, f)
            return neutral.compare(a, b)

    def test_equal_but_hash_tick_jobs(self):
        new = copy.deepcopy(BASE)
        new["per_match"][0]["hash"] = "0xff"
        new["tick_p99_ms"] = {"all": 9.9}
        new["jobs"] = 6
        lines, ok = self.run_dirs(BASE, new)
        self.assertTrue(ok, lines)
        self.assertEqual(lines, ["skirmish40.json: same 2/2"])

    def test_a_changed_match_field_fails(self):
        new = copy.deepcopy(BASE)
        new["per_match"][1]["length_s"] = 982.9
        lines, ok = self.run_dirs(BASE, new)
        self.assertFalse(ok)
        self.assertIn("same 1/2", lines[0])
        self.assertIn("length_s", lines[1])

    def test_new_zero_counter_passes_nonzero_fails(self):
        new = copy.deepcopy(BASE)
        new["factions"]["germany"]["s10"] = {"pieces_built": 0, "wire": [0, 0]}
        new["per_match"][0]["s10_pieces"] = 0
        self.assertTrue(self.run_dirs(BASE, new)[1])
        new["factions"]["germany"]["s10"]["pieces_built"] = 1
        lines, ok = self.run_dirs(BASE, new)
        self.assertFalse(ok)
        self.assertIn("only in NEW", "\n".join(lines))

    def test_s10c_keys_skipped_only_when_base_lacks_them(self):
        # S10c-ai Build A: observer blocks read state, so they are not zero
        # with the switches off. Control: the same block under another name
        # fails (only in NEW and not zero).
        new = copy.deepcopy(BASE)
        new["factions"]["germany"]["s10c"] = {"mines_seen": 7}
        new["factions"]["germany"]["s10c_derived"] = {"front_calm_pct": 50.0}
        lines, ok = self.run_dirs(BASE, new)
        self.assertTrue(ok, lines)
        other = copy.deepcopy(BASE)
        other["factions"]["germany"]["x10c"] = {"mines_seen": 7}
        lines, ok = self.run_dirs(BASE, other)
        self.assertFalse(ok, lines)
        # Both have it: compared.
        base = copy.deepcopy(new)
        changed = copy.deepcopy(new)
        changed["factions"]["germany"]["s10c"]["mines_seen"] = 8
        lines, ok = self.run_dirs(base, changed)
        self.assertFalse(ok, lines)
        self.assertIn("factions.germany.s10c.mines_seen: 7 -> 8", lines[1])

    def test_neutral_skips_s11a1_keys_the_base_lacks(self):
        # S11a1 Build B: the `s11a1` blocks are skipped when BASE lacks them.
        # Control: the same block under another name fails.
        new = copy.deepcopy(BASE)
        new["factions"]["germany"]["s11a1"] = {"guns_bought": 1}
        new["factions"]["germany"]["s11a1_derived"] = {"gun_observed_pct": 50.0}
        lines, ok = self.run_dirs(BASE, new)
        self.assertTrue(ok, lines)
        other = copy.deepcopy(BASE)
        other["factions"]["germany"]["x11a1"] = {"guns_bought": 1}
        self.assertFalse(self.run_dirs(BASE, other)[1])

    def test_grown_list_with_zero_tail_is_equal(self):
        # S11a1 Build B: `s5b.refused` grew 7 -> 9. Control: a non-zero
        # tail, or a shorter list, still fails.
        base = copy.deepcopy(BASE)
        base["factions"]["germany"]["s5b"] = {"refused": [1, 0, 2]}
        new = copy.deepcopy(base)
        new["factions"]["germany"]["s5b"]["refused"] = [1, 0, 2, 0, 0]
        lines, ok = self.run_dirs(base, new)
        self.assertTrue(ok, lines)
        new["factions"]["germany"]["s5b"]["refused"] = [1, 0, 2, 0, 1]
        self.assertFalse(self.run_dirs(base, new)[1])
        new["factions"]["germany"]["s5b"]["refused"] = [1, 0]
        self.assertFalse(self.run_dirs(base, new)[1])

    def test_key_only_in_base_fails(self):
        new = copy.deepcopy(BASE)
        del new["wins"]["usa"]
        lines, ok = self.run_dirs(BASE, new)
        self.assertFalse(ok)
        self.assertIn("only in BASE", "\n".join(lines))

    def test_missing_match_fails(self):
        new = copy.deepcopy(BASE)
        new["per_match"].pop()
        self.assertFalse(self.run_dirs(BASE, new)[1])

    def test_other_json_new_rows_allowed_base_rows_equal(self):
        base = {"rows": [{"case": 1, "pass": True}], "pass": True}
        new = {"rows": [{"case": 1, "pass": True}], "pass": True, "s10a": {"wire": 3}}
        lines, ok = self.run_dirs(base, new, "fortify.json")
        self.assertTrue(ok, lines)
        new["rows"][0]["pass"] = False
        self.assertFalse(self.run_dirs(base, new, "fortify.json")[1])

    def test_no_common_file_fails(self):
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            self.assertFalse(neutral.compare(a, b)[1])

    def test_skirmish_set_missing_from_new_fails(self):
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            for d in (a, b):
                with open(os.path.join(d, "skirmish40.json"), "w") as f:
                    json.dump(BASE, f)
            with open(os.path.join(a, "skirmish160.json"), "w") as f:
                json.dump(BASE, f)
            lines, ok = neutral.compare(a, b)
            self.assertFalse(ok, lines)
            self.assertIn("skirmish160.json: only in BASE (missing from NEW)", lines)
            self.assertIn("skirmish40.json: same 2/2", lines)
            # Other JSON only in BASE is not compared (and does not fail).
            os.remove(os.path.join(a, "skirmish160.json"))
            with open(os.path.join(a, "fortify.json"), "w") as f:
                json.dump({"pass": True}, f)
            self.assertTrue(neutral.compare(a, b)[1])

    def test_overrides_header_is_a_note_not_a_difference(self):
        base = copy.deepcopy(BASE)
        base["overrides"] = []
        new = copy.deepcopy(base)
        new["overrides"] = ["override all_off.toml: ai.fort.works: 1 -> 0", "x"]
        lines, ok = self.run_dirs(base, new)
        self.assertTrue(ok, lines)
        self.assertEqual(lines, ["skirmish40.json: same 2/2 (note: overrides 0 -> 2 items)"])
        # Missing in BASE (older sets) is a note too.
        del base["overrides"]
        self.assertTrue(self.run_dirs(base, new)[1])
        # Control: a match difference still fails, and keeps the note.
        new["per_match"][0]["length_s"] = 1.0
        lines, ok = self.run_dirs(base, new)
        self.assertFalse(ok)
        self.assertIn("DIFFERENT, same 1/2 (note: overrides", lines[0])
        # Control: another header field is still compared.
        new = copy.deepcopy(base)
        new["matches"] = 3
        self.assertFalse(self.run_dirs(base, new)[1])

    def test_first_five_differences_only(self):
        new = copy.deepcopy(BASE)
        for m in new["per_match"]:
            m.update(result="lost", winner_base=7, length_s=1.0, he_share_pct=0.0)
        lines, ok = self.run_dirs(BASE, new)
        self.assertFalse(ok)
        self.assertEqual(len(lines), 6)


if __name__ == "__main__":
    unittest.main()
