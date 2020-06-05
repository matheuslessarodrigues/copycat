# copycat

A simple clipboard cli interface for windows (could be crossplatform if someone can implement getting image from clipboard for other platforms).

Pipe into it to copy. Pipe from it to paste.

```
$ echo "copy this!" | copycat
```

```
$ copycat
> copy this!
```

```
$ copycat | grep .
> copy this!
```
