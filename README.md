# copycat

A simple crossplatform clipboard cli interface.

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
