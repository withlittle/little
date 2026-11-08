================
reStructuredText
================

.. meta::
   :description: Build each reStructuredText file into a page, without docutils, Sphinx, or Python, with the restructuredtext plugin.

``restructuredtext``, one of `little's own plugins <using-plugins.md#littles-own-plugins>`__, makes each `reStructuredText <https://docutils.sourceforge.io/rst.html>`__ file in ``docs/``, ``.rst``, a `page <using-plugins.md#pages-in-other-formats>`__, without docutils, Sphinx, or Python.
Name it in your site's `plugins <using-plugins.md#naming-a-plugin>`__:

.. code:: yaml

   # little.yml
   plugins:
     restructuredtext: https://github.com/withlittle/little/releases/download/restructuredtext-v0.1.0/little_restructuredtext.wasm

It has no settings.
This page is one reStructuredText file, ``docs/plugins/restructuredtext.rst``.

The title
=========

A title alone at the top of the file, in a style of its own, is the page's title and its ``#`` heading, and a lone title just below it, at the next level, is its subtitle, in italics.
A ``meta`` directive's ``:description:`` is the page's description:

.. code:: rst

   ================
   reStructuredText
   ================

   .. meta::
      :description: Build each reStructuredText file into a page.

A ``title`` directive, ``.. title:: Text``, is the page's title, for the nav and the browser's tab, without changing its heading.
A page can also start with `frontmatter <../writing/frontmatter.md#in-another-format>`__, above its title, which goes over its title and description.

A field list just below the title, like ``:Author:`` or ``:Version:``, is a description list, except that ``:Dedication:`` and ``:Abstract:`` are each their name, in bold, above their text.

Text
====

reStructuredText's markup is Markdown's where Markdown has it: *emphasis*, **strong emphasis**, and ``inline literals``, which are code.
Where it hasn't, it's HTML: the ``sub`` role, for H\ :sub:`2`\ O, is ``<sub>``, and the ``sup`` role, for E = mc\ :sup:`2`, ``<sup>``, or Markdown, where your site turns on the `extensions <../writing/markdown.md#choosing-markdowns-extensions>`__ ``subscript`` and ``superscript``.

Text in single backquotes, the default role, names a title, like `A Tale of Two Cities`, in italics, and the ``default-role`` directive changes the default.
These roles are read too:

* ``emphasis``, ``strong``, ``literal``, and ``code`` are the markup they name.
* ``math`` is `math`_.
* ``pep`` and ``rfc`` link to the PEP or RFC they number.
* ``abbreviation`` and ``acronym`` are their text.

A role you make with the ``role`` directive is the role it's based on, or its text, and one based on ``raw``, for ``html``, is HTML, as it is.
A role the plugin doesn't read, like Sphinx's ``ref`` or ``doc``, keeps its text, and is warned of.

A backslash escapes markup, like ``\*``, and a backslash before a space joins what's either side of it, as in ``H\ :sub:`2`\ O``.

A substitution, like ``|rarr|``, is what its definition makes it.
Here, ``.. |rarr| unicode:: U+2192`` makes it |rarr|.
A definition can make it:

* Text, with ``replace``.
* Characters, with ``unicode``.
* An image, with ``image``.
* The date the site is built, with ``date``.

.. |rarr| unicode:: U+2192 .. rightwards arrow

Sections and links
==================

A section's title, underlined, or overlined and underlined, with a punctuation character, like ``=``, ``-``, or ``~``, is a heading.
Its level is the order its style is first used in: on this page, under the title, a title underlined with ``=`` is ``##``, the next style ``###``, and so on, to ``######``.
A link to it leads to the ``#fragment`` little gives a heading in Markdown, and docutils' id for it, where that differs, like ``#uber-strasze`` for Über Straße, leads to it too.

A URL, like https://docutils.sourceforge.io, or an email address, is a link, and so is ```text <https://example.com>`_``, which ```text`_`` links to again.
One with two underscores, ```text <https://example.com>`__``, is a link that nothing links to again.

A target, ``.. _name: https://example.com``, gives ``name_`` its URL, and one without a URL, above a block, leads to the block.
A section's title is a target too, so ```The title`_`` leads to `The title`_.
A link to a page's file, like ```Markdown <../writing/markdown.md>`__``, or to another ``.rst`` file, leads to its page.

Lists
=====

Each of reStructuredText's lists is the Markdown list nearest it:

Bullets
   ``-``, ``*``, or ``+``, nested by indenting them, are a bulleted list.

Numbers
   ``1.``, ``1)``, ``(1)``, or ``#.``, which numbers itself, are a numbered list.
   It starts at its first number.

Terms
   A term, with what it means indented below it, like these, is a description list.

Fields and options
   A field list, ``:Field: text``, and an option list, ``-o  text``, are description lists too, as docutils writes them.

A numbered list's letters or Roman numerals, like ``c.`` or ``iv.``, are numbers, like 3 or 4, as Markdown numbers lists only with numbers.

Code
====

A literal block, indented below a paragraph that ends in ``::``, is a code block with no language, and its ``::`` is a colon, as here::

   A literal block shows *markup* as it is.

A ``code`` directive, or ``code-block``, or ``sourcecode``, is a code block in the language it names:

.. code:: rst

   .. code:: python

      print("Hello")

A code block goes to `plugins <using-plugins.md#code-blocks>`__ that render its language, and a ``mermaid`` one is a `diagram <../writing/diagrams.md>`__.
A doctest block, a Python session that starts with ``>>>``, is code in ``pycon``, and a ``parsed-literal`` is code, without its markup.

Admonitions
===========

.. note::

   An admonition is the nearest of GitHub's five `alerts <../writing/markdown.md#callouts>`__, like this note.

.. list-table:: Which alert each admonition is
   :header-rows: 1

   * - Admonition
     - Alert
   * - ``note``, and ``admonition``, with its own title
     - Note
   * - ``tip``, and ``hint``, titled Hint
     - Tip
   * - ``important``
     - Important
   * - ``warning``, and ``attention`` and ``caution``, each titled with its name
     - Warning
   * - ``danger`` and ``error``, each titled with its name
     - Caution

Other blocks
============

.. list-table:: What other blocks are
   :header-rows: 1
   :widths: 2 3

   * - Block
     - In the page
   * - Block quote, an indented paragraph
     - A quote, ending with its attribution, after ``--``
   * - Line block, lines that start with ``|``
     - A paragraph that keeps its line breaks
   * - ``epigraph``, ``highlights``, and ``pull-quote``
     - A quote
   * - ``topic`` and ``sidebar``
     - Its title, in bold, and a sidebar's subtitle, in italics, above what it holds
   * - ``rubric``
     - Its text, in bold
   * - ``compound`` and ``container``
     - What they hold
   * - ``raw``, for ``html``
     - Its HTML, as it is, and nothing for another format
   * - Transition, a line of ``-`` or another punctuation character
     - A rule
   * - ``contents``
     - Nothing, as the theme has its own
   * - Comment, ``..`` and its text, and ``class``
     - Nothing

A table's title, like this one's, is a paragraph in italics, above it.

Tables
======

A grid table, drawn with ``+``, ``-``, and ``|``, and a simple table, of columns between lines of ``=``, are each one of GitHub's tables, with their header row, if they have one.
A ``table`` directive's table, a ``csv-table``, with its data in the directive, and a ``list-table``, like those above, with ``:header-rows: 1``, are too.

Markdown's tables can't merge cells, or hold blocks, so a cell that spans columns or rows is written in its first place, with empty cells after it, and a cell's blocks, like a list, are run together, a line each.
Each is warned of.

Images
======

An image, ``.. image:: file.png``, with its ``:alt:`` text, is in a paragraph of its own, and one with a ``:target:`` is a link.
A figure, ``.. figure:: file.png``, is its image, then its caption, in italics, then its legend.
An image in a substitution, ``.. |name| image:: file.png``, is in the line, where ``|name|`` is.

Without ``:alt:``, an image's alt text is its path.
Its size and alignment, like ``:width:``, are left out, for your theme's CSS.

Footnotes
=========

A footnote, ``[#]_``, with its text in ``.. [#]``, is numbered through the page, like this one\ [#]_, as are those with numbers, ``[1]_``, names, ``[#name]_``, or symbols, ``[*]_``, and citations, ``[CIT2002]_``.
They're numbered in the order they're first referred to, so their labels are lost.

.. [#] Its text is at the foot of the page.

Math
====

Math is TeX, written as `your site chooses <../writing/math.md#choosing-how-math-is-written>`__: the ``math`` role in a line, like :math:`e^{i\pi} + 1 = 0`, or the ``math`` directive, on its own, where each formula, between blank lines, is a block of its own.

What's different
================

The plugin reads reStructuredText as docutils does, without Sphinx's additions, and leaves out what docutils' HTML leaves out, like comments, except that:

* An include, ``.. include:: file.rst``, isn't followed, and nor is a ``raw`` or ``csv-table`` directive's ``:file:`` or ``:url:``. Each is left out, and warned of.
* A directive the plugin doesn't read, like Sphinx's ``toctree``, or one docutils doesn't accept, like a ``code-block`` with Sphinx's ``:linenos:``, is kept as code, as you wrote it, and warned of.
