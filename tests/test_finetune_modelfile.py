import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("make_modelfile", ROOT / "research" / "finetune" / "make_modelfile.py")
make_modelfile = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(make_modelfile)


class ModelfileTest(unittest.TestCase):
    def test_embeds_guide_as_system_prompt(self) -> None:
        guide = (ROOT / "spec" / "LLM_GUIDE.md").read_text(encoding="utf-8")
        text = make_modelfile.modelfile(Path("model.gguf"), guide, 0.2, 8192)
        self.assertTrue(text.startswith("FROM "))
        self.assertIn("PARAMETER temperature 0.2", text)
        self.assertIn("PARAMETER num_ctx 8192", text)
        self.assertIn(guide.strip(), text)
        # The system block is closed exactly once, after the guide.
        self.assertEqual(text.count('"""'), 2)
        self.assertTrue(text.endswith('"""\n'))

    def test_rejects_guides_that_would_close_the_block(self) -> None:
        with self.assertRaises(ValueError):
            make_modelfile.modelfile(Path("model.gguf"), 'a """ b', 0.2, 8192)


if __name__ == "__main__":
    unittest.main()
