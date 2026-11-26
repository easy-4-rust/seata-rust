from contextlib import redirect_stdout
import io
from pathlib import Path
import sys
sys.path.insert(0,'tools/formal-tc-lock-coverage')
import live_guard as g
command = g.reg.command
result_before_loss = {}
stdout = io.StringIO()
def suppress_create_id(args, env, **kwargs):
    result = command(args, env, **kwargs)
    if list(args[:2]) == ['docker','create']:
        result_before_loss.update(container_id=result.decode().strip(), name=args[args.index('--name')+1],
                                  label=args[args.index('--label')+1])
        raise RuntimeError('fixture-injected loss of Docker create result')
    return result
g.reg.command = suppress_create_id
try:
    with redirect_stdout(stdout):
        g.run('1.99')
except RuntimeError as error:
    assert str(error) == 'fixture-injected loss of Docker create result'
    out = Path(stdout.getvalue().splitlines()[0])
    query = ['docker','ps','-aq','--no-trunc','--filter','label='+result_before_loss['label']]
    after = command(query,g.os.environ).decode()
    assert result_before_loss['container_id'] and after==''
    g.save(out/'fault-injection.json',dict(injected_error=str(error),create=result_before_loss,post_cleanup_query=query,
                                          post_cleanup_stdout=after,owned_container_removed=True))
    g.put(out/'fault-injection.py',Path(__file__).read_bytes())
    g.save(out/'catalog.json',dict(members=[dict(path=p.name,sha256=g.sha(p)) for p in sorted(out.iterdir())]))
    print(out.name, g.sha(out/'catalog.json'), result_before_loss['container_id'])
else:
    raise AssertionError('missed intentional create-result failure')