"""Contract tests use synthetic data only; never native acceptance evidence."""
import copy
import gzip
import json
from pathlib import Path
import tempfile
import unittest

import rust_collector as collector
from rust_native import native_rows


class NativeIntegrityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='arc-quality-contract-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / 'source'
        (self.source / 'backend/src').mkdir(parents=True)
        (self.source / 'backend/src/lib.rs').write_text('pub fn example() {}\n')
        self.path = 'backend/src/lib.rs'
        self.inventory = {self.path: collector.sha((self.source / self.path).read_bytes())}
        self.llvm = {'type': 'llvm.coverage.json.export', 'data': [{'files': [{
            'filename': str(self.source / self.path),
            'summary': {'lines': {'covered': 0, 'count': 1}, 'regions': {'covered': 0, 'count': 1}}}]}]}
        data = collector.canonical(self.llvm)
        (self.root / 'llvm.json.gz').write_bytes(gzip.compress(data))
        (self.root / 'tests.log').write_text('synthetic unit-test fixture, not a native run\n')
        (self.root / 'execution.json').write_text(json.dumps({'commit': 'fixture', 'run': 'fixture', 'exit_code': 0}))
        self.raw = {'schema': 'arc-admin-rust-native/v1', 'commit': 'fixture', 'run': 'fixture',
                    'source_inventory': self.inventory,
                    'coverage': native_rows(self.llvm, self.source, self.inventory),
                    'risk_state': 'measurement_error', 'risk_reason': 'fixture',
                    'llvm_sha256': collector.sha(data),
                    'tests_sha256': collector.sha((self.root / 'tests.log').read_bytes()),
                    'execution_sha256': collector.sha((self.root / 'execution.json').read_bytes())}

    def validate(self):
        data = collector.canonical(self.raw)
        (self.root / 'native.json').write_bytes(data)
        return collector.validate_native(self.root, collector.sha(data), self.source)

    def test_valid_zero_hits_are_not_successful_coverage(self):
        self.assertEqual(self.validate()['coverage']['files'][self.path]['lines'], {'covered': 0, 'total': 1})

    def test_missing_source_row_is_not_fabricated_zero_denominator(self):
        self.llvm['data'][0]['files'] = []
        self.assertEqual(native_rows(self.llvm, self.source, self.inventory), {'files': {}, 'missing': [self.path]})

    def test_duplicate_native_row_rejected(self):
        self.llvm['data'][0]['files'] *= 2
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            native_rows(self.llvm, self.source, self.inventory)

    def test_boolean_counter_rejected(self):
        self.llvm['data'][0]['files'][0]['summary']['lines']['covered'] = True
        with self.assertRaisesRegex(ValueError, 'counters'):
            native_rows(self.llvm, self.source, self.inventory)

    def test_favorable_summary_without_raw_change_rejected(self):
        self.raw['coverage']['files'][self.path]['lines']['covered'] = 1
        with self.assertRaisesRegex(ValueError, 'raw LLVM'):
            self.validate()

    def test_source_mutation_rejected(self):
        (self.source / self.path).write_text('changed\n')
        with self.assertRaisesRegex(ValueError, 'digest mismatch'):
            self.validate()

    def test_extra_source_rejected(self):
        (self.source / 'backend/src/extra.rs').write_text('pub fn extra() {}')
        with self.assertRaisesRegex(ValueError, 'inventory changed'):
            self.validate()

    def test_failed_test_run_cannot_produce_evidence(self):
        (self.root / 'execution.json').write_text(json.dumps({'commit': 'fixture', 'run': 'fixture', 'exit_code': 1}))
        self.raw['execution_sha256'] = collector.sha((self.root / 'execution.json').read_bytes())
        with self.assertRaisesRegex(ValueError, 'did not pass'):
            self.validate()

    def test_unknown_crap_support_rejected(self):
        self.raw['risk_state'] = 'supported'
        with self.assertRaisesRegex(ValueError, 'uncertified CRAP'):
            self.validate()

    def test_duplicate_json_keys_rejected(self):
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            collector.decode('{"metric": 0, "metric": 1}')

    def test_retained_export_can_move_without_rewriting_native_bytes(self):
        self.llvm['data'][0]['files'][0]['filename'] = '/original/runner/' + self.path
        data = collector.canonical(self.llvm)
        self.raw['llvm_sha256'] = collector.sha(data)
        (self.root / 'llvm.json.gz').write_bytes(gzip.compress(data))
        self.assertEqual(self.validate()['source_inventory'], self.inventory)


if __name__ == '__main__':
    unittest.main()
