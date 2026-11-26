"""Retain the certified normalized AT guards while adding FILE-specific evidence gates."""
import importlib.util,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('file_timeout_normalized_parent',ROOT/'tools/go-java-v1-real-commit-timeout/verify.py')
parent=importlib.util.module_from_spec(spec);spec.loader.exec_module(parent)
def validate(value):
 parent.validate(value)
 try:
  storage=value['file_storage']
  if storage['mode']!='file' or sorted(storage['tables'])!=['accounts','commit_account','commit_reservation','undo_log']:raise RuntimeError('TC storage must be FILE; MySQL must contain only real business/Undo/TCC tables')
  journals=storage['journals'];restores=storage['restore_inputs']
  if [row['stage'] for row in journals]!=['initial','operator_loss','operator_replay'] or [row['stage'] for row in restores]!=['operator_loss','operator_replay']:raise RuntimeError('Incomplete or reordered stopped FILE lifecycle')
  for record in journals+restores:
   files=record['files']
   if not files or len({row['path'] for row in files})!=len(files):raise RuntimeError('Missing or ambiguous actual FILE journal')
   for row in files:
    path=row['path']
    if not path.startswith('28091/') or '..' in Path(path).parts or Path(path).is_absolute() or not re.fullmatch('[0-9a-f]{64}',row['sha256']) or not isinstance(row['bytes'],int) or row['bytes']<0:raise RuntimeError('Invalid actual journal file metadata')
   if not any(row['path']=='28091/root.data' and row['bytes']>0 for row in files):raise RuntimeError('Actual FILE root journal missing or empty')
  for index in range(2):
   if journals[index]['files']!=restores[index]['files']:raise RuntimeError('Restart did not consume the exact previously stopped FILE journal')
 except (KeyError,TypeError,ValueError) as error:raise RuntimeError('Incomplete FILE recovery evidence') from error
 return value
