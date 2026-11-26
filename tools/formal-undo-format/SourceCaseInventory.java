import com.sun.source.tree.AnnotationTree;
import com.sun.source.tree.ClassTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreeScanner;
import com.sun.source.util.Trees;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HexFormat;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;

/** JDK AST只读库存；声明方法不是已展开/已执行的具体案例。 */
public final class SourceCaseInventory {
    private static final Set<String> TESTS=Set.of("Test","ParameterizedTest","RepeatedTest","TestFactory","TestTemplate","Theory");
    public static void main(String[] args) throws Exception {
        List<Path> paths=new ArrayList<>();
        for(int i=1;i<args.length;i++) {
            try(java.util.stream.Stream<Path> stream=Files.walk(Path.of(args[i]))) {
                stream.filter(p->p.toString().endsWith(".java")).forEach(paths::add);
            }
        }
        paths.sort(Comparator.naturalOrder());
        JavaCompiler compiler=Objects.requireNonNull(ToolProvider.getSystemJavaCompiler());
        DiagnosticCollector<JavaFileObject> diagnostics=new DiagnosticCollector<>();
        List<String> output=new ArrayList<>();
        try(StandardJavaFileManager manager=compiler.getStandardFileManager(diagnostics,null,StandardCharsets.UTF_8)) {
            JavacTask task=(JavacTask)compiler.getTask(null,manager,diagnostics,List.of("-proc:none"),null,manager.getJavaFileObjectsFromPaths(paths));
            Trees trees=Trees.instance(task);
            for(CompilationUnitTree unit:task.parse()) {
                String text=unit.getSourceFile().getCharContent(true).toString();
                String file=Path.of(unit.getSourceFile().toUri()).toString();
                String pkg=Objects.isNull(unit.getPackageName())?"":unit.getPackageName().toString();
                new TreeScanner<Void,Void>() {
                    private final List<String> owners=new ArrayList<>();
                    private final List<String> parents=new ArrayList<>();
                    @Override public Void visitClass(ClassTree node,Void unused) {
                        owners.add(node.getSimpleName().toString());
                        parents.add(Objects.isNull(node.getExtendsClause())?"":node.getExtendsClause().toString());
                        super.visitClass(node,unused);owners.remove(owners.size()-1);parents.remove(parents.size()-1);return null;
                    }
                    @Override public Void visitMethod(MethodTree node,Void unused) {
                        List<String> annotations=new ArrayList<>();boolean selected=false;
                        for(AnnotationTree annotation:node.getModifiers().getAnnotations()) {
                            String name=annotation.getAnnotationType().toString();name=name.substring(name.lastIndexOf('.')+1);
                            if(TESTS.contains(name))selected=true;
                            annotations.add(annotation.toString());
                        }
                        if(selected) {
                            long start=trees.getSourcePositions().getStartPosition(unit,node),end=trees.getSourcePositions().getEndPosition(unit,node);
                            try {
                                String digest=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(text.substring((int)start,(int)end).getBytes(StandardCharsets.UTF_8)));
                                String owner=pkg+"."+String.join("$",owners);
                                String signature=node.getName()+"("+String.join(",",node.getParameters().stream().map(p->p.getType().toString()).toList())+")";
                                output.add("{\"source\":"+quote(file)+",\"declaring_class\":"+quote(owner)+",\"method\":"+quote(node.getName().toString())
                                    +",\"signature\":"+quote(signature)+",\"line\":"+unit.getLineMap().getLineNumber(start)+",\"end_line\":"+unit.getLineMap().getLineNumber(end)
                                    +",\"annotations\":"+quote(String.join("\n",annotations))+",\"direct_superclass_syntax\":"+quote(parents.get(parents.size()-1))
                                    +",\"method_checkout_sha256\":"+quote(digest)+",\"concrete_cases_expanded\":false,\"Rust_disposition\":\"MISSING\",\"differential\":\"NOT_RUN\"}");
                            } catch(Exception error) {throw new IllegalStateException(error);}
                        }
                        return super.visitMethod(node,unused);
                    }
                }.scan(unit,null);
            }
        }
        for(Diagnostic<? extends JavaFileObject> diagnostic:diagnostics.getDiagnostics()) {
            if(diagnostic.getKind()==Diagnostic.Kind.ERROR)throw new IllegalStateException("Source inventory parse error: "+diagnostic);
        }
        Files.write(Path.of(args[0]),output,StandardCharsets.UTF_8);
        System.out.println("source_files="+paths.size()+" declared_test_methods="+output.size()+" concrete_expansion=false execution=false");
    }
    private static String quote(String value) {
        StringBuilder result=new StringBuilder("\"");
        for(char c:value.toCharArray()) {
            if(c=='\"' || c=='\\')result.append('\\').append(c);
            else if(c<32 || c>=127)result.append(String.format("\\u%04x",(int)c));
            else result.append(c);
        }
        return result.append('\"').toString();
    }
}
