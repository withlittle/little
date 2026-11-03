================
reStructuredText
================

.. meta::
   :description: Build each reStructuredText file into a page, without docutils, Sphinx, or Python, with the restructuredtext plugin.

little's repository has a plugin,
`restructuredtext <https://github.com/withlittle/little/tree/main/plugins/restructuredtext>`__,
that makes each `reStructuredText <https://docutils.sourceforge.io/rst.html>`__ file (``.rst``) in ``docs/`` a page, reading reStructuredText itself, as docutils does, without docutils, Sphinx, or Python, or converting it first.
This page is one, ``docs/plugins/restructuredtext.rst``, and a link to it from Markdown, like ``[reStructuredText](restructuredtext.rst)``, leads to its page.

Getting it
==========

Each version of ``restructuredtext`` is released in little's repository, as a ``.wasm`` file, about 570 KB, which a site names by its URL, with the version in it::

   https://github.com/withlittle/little/releases/download/restructuredtext-v0.1.0/little_restructuredtext.wasm

little downloads it in the site's first build, and uses that copy from then on, as for `any plugin <using-plugins.md#naming-a-plugin>`__, so to update it, change the URL to a later version's.
Its releases, among `little's <https://github.com/withlittle/little/releases>`__, list what changed in each version, and the version of little's `hooks <../reference/plugin-hooks.md>`__ it's built for, and each carries a ``THIRD-PARTY-NOTICES.md``, with the licences of the crates it's built from.
To build it yourself, see its
`README <https://github.com/withlittle/little/tree/main/plugins/restructuredtext#building-it>`__.

Using it
========

Name it in your site's ``little.yml``, and each reStructuredText file in ``docs/`` is a page:

.. code:: yaml

   # little.yml
   plugins:
     restructuredtext: https://github.com/withlittle/little/releases/download/restructuredtext-v0.1.0/little_restructuredtext.wasm

It has no settings.
Without it, a reStructuredText file is copied into the site as it is, as any other file is.
A folder's ``README.rst`` is its page, as a ``README.md`` is.

The title
=========

A title alone at the top of the file, in a style of its own, like this page's, overlined and underlined with ``=``, is the page's title, and its ``#`` heading, as docutils makes it the document's title, and a lone title just below it, at the next level, is its subtitle, in italics.
A ``meta`` directive's ``:description:`` is the page's description, and a ``title`` directive, ``.. title:: Text``, is the page's title, for the nav and the browser's tab, without changing its heading:

.. code:: rst

   ================
   reStructuredText
   ================

   .. meta::
      :description: Build each reStructuredText file into a page.

A field list just below the title, like ``:Author:`` or ``:Version:``, is a description list, as docutils' bibliographic fields are, but for ``:Dedication:`` and ``:Abstract:``, each its name, in bold, above its text.

A page can start with `frontmatter <../writing/frontmatter.md#in-another-format>`__ too, between ``---`` lines, above its title, as a Markdown page can, which goes over its title and description, for any of little's keys, like ``search: false``:

.. code:: rst

   ---
   search: false
   ---
   =============
   Release notes
   =============

   .. meta::
      :description: What changed in each version.

Text
====

reStructuredText's markup is Markdown's where Markdown has it: *emphasis*, **strong emphasis**, and ``inline literals``, which are code.
Where it hasn't, it's HTML: the ``sub`` role, for H\ :sub:`2`\ O, is ``<sub>``, and the ``sup`` role, for E = mc\ :sup:`2`, ``<sup>``.
With the `extensions <../writing/markdown.md#choosing-markdowns-extensions>`__ for them on, ``subscript`` and ``superscript``, they're Markdown, and with neither those nor `HTML <../writing/markdown.md#html>`__, they're plain text, and warned of.

Text in single backquotes, the default role, names a title, like `A Tale of Two Cities`, in italics.
The roles ``emphasis``, ``strong``, ``literal``, and ``code`` are the markup they name, ``math`` is `math`_, ``pep`` and ``rfc`` link to the PEP or RFC they number, and ``abbreviation`` and ``acronym`` are their text.
A role you make with the ``role`` directive is the role it's based on, or its text, and one based on ``raw``, for ``html``, is HTML, as it is; ``default-role`` changes the default.
A role the plugin doesn't read, like Sphinx's ``ref``, keeps its text, and is warned of.

Text shows as you write it, even where Markdown would read it as markup, like the * in 2 * 3, or :smile:.
A backslash escapes markup, like ``\*``, and a backslash before a space joins what's either side of it, as in ``H\ :sub:`2`\ O``.
A substitution, like ``|rarr|``, is what its definition, like ``.. |rarr| unicode:: U+2192``, makes it, |rarr| here: text, with ``replace``, characters, with ``unicode``, an image, with ``image``, or the date the site is built, with ``date``.

.. |rarr| unicode:: U+2192 .. rightwards arrow

Sections and links
==================

A section's title, underlined, or overlined and underlined, with a punctuation character, like ``=``, ``-``, or ``~``, is a heading.
Its level is the order its style is first used in: on this page, under the title, a title underlined with ``=`` is ``##``, the next style ``###``, and so on, to ``######``.
A link to it leads to the ``#fragment`` little gives a heading in Markdown, and docutils' id for it, like ``#uber-strasze`` for Über Straße, leads to it too, where the two differ.

A URL, like https://docutils.sourceforge.io, is a link, and so is an email address, and ```text <https://example.com>`_``, which ```text`_`` links to again, or ```text <https://example.com>`__``, with two underscores, which nothing can.
A target, ``.. _name: https://example.com``, gives ``name_`` its URL, and one without a URL, above a block, leads to the block.
A section's title is a target too, so ```Using it`_`` leads to `Using it`_.
A link to a page's file, like ```Markdown <../writing/markdown.md>`__``, leads to its page, as little leads a link to one, and so does one to another ``.rst`` file.

Lists
=====

Each of reStructuredText's lists is the Markdown list nearest it:

Bullets
   ``-``, ``*``, or ``+``, nested by indenting them, are a bulleted list.

Numbers
   ``1.``, ``1)``, or ``(1)``, or ``#.``, which numbers itself, are a numbered list, which starts at its first number.

Terms
   A term, with what it means indented below it, like these, is a description list, in HTML, unless the ``description_lists`` extension is on.

Fields and options
   A field list, ``:Field: text``, and an option list, ``-o  text``, are description lists too, as docutils writes them.

A numbered list's letters or Roman numerals, like ``c.`` or ``iv.``, are numbers, like 3 or 4, as Markdown numbers lists only with numbers.

Code
====

A literal block, indented below a paragraph that ends in ``::``, is a code block with no language, as the URL in `Getting it`_ is, and its ``::`` a colon.
A ``code`` directive, or ``code-block``, or ``sourcecode``, is a code block in the language it names:

.. code:: rst

   .. code:: python

      print("Hello")

Like a Markdown page's, a code block goes to `plugins <using-plugins.md#code-blocks>`__ that render its language, and a ``mermaid`` one is a `diagram <../writing/diagrams.md>`__.
A doctest block, a Python session that starts with ``>>>``, is code in ``pycon``, and a ``parsed-literal`` is code, without its markup, which is warned of.

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

Where your site turns the ``alerts`` extension off, an admonition is a quote that starts with its name, or its title, in bold.

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
     - Its HTML, as it is, unless your site turns `HTML <../writing/markdown.md#html>`__ off, and nothing for another format
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
So are a ``table`` directive's table, a ``csv-table``, with its data in the directive, and a ``list-table``, like those above, with ``:header-rows: 1``.
A table without a header row has an empty one, as Markdown's tables need one, which the built-in theme hides.

A cell that spans columns or rows is written in its first place, with empty cells after it, header rows after the first are joined to the body, and a cell's blocks, like a list, are run together, a line each, as Markdown's tables can't hold more; each is warned of.

Images
======

An image, ``.. image:: file.png``, with its ``:alt:`` text, is in a paragraph of its own, and one with a ``:target:`` is a link.
A figure, ``.. figure:: file.png``, is its image, then its caption, in italics, then its legend.
An image in a substitution, ``.. |name| image:: file.png``, is in the line, where ``|name|`` is.
Without ``:alt:``, an image's alt text is its path, as in docutils' HTML, and its size and alignment, like ``:width:``, are left out, for your theme's CSS.

Footnotes
=========

A footnote, ``[#]_``, with its text in ``.. [#]``, is numbered through the page, like this one\ [#]_.
So are those with numbers, ``[1]_``, names, ``[#name]_``, or symbols, ``[*]_``, and citations, ``[CIT2002]_``, in the order they're first referred to, so their labels are lost.
A footnote nothing refers to is left out, and warned of.

.. [#] Its text is at the foot of the page.

Math
====

Math is TeX, written as `your site chooses <../writing/math.md#choosing-how-math-is-written>`__: the ``math`` role, in a line, like :math:`e^{i\pi} + 1 = 0`, or the ``math`` directive, on its own, each formula in it, between blank lines, a block of its own.

What's different
================

The plugin reads reStructuredText as docutils does, without Sphinx's additions, and leaves out what docutils' HTML leaves out, like comments, but:

* an include, ``.. include:: file.rst``, isn't followed, as a plugin reads only the file it's given, nor is a ``raw`` or ``csv-table`` directive's ``:file:`` or ``:url:``: each is warned of, and left out;
* a directive the plugin doesn't read, like Sphinx's ``toctree``, or one docutils doesn't accept, like a ``code-block`` with Sphinx's ``:linenos:``, is kept as code, as you wrote it, and warned of, with its line;
* a role the plugin doesn't read, like Sphinx's ``ref`` or ``doc``, keeps its text, and is warned of;
* ``sectnum``, ``header``, ``footer``, and ``target-notes`` are warned of, and left out, as the plugin doesn't number sections or list targets' URLs, and the theme writes the page's header and footer;
* a title, in the default role, is in italics, not ``<cite>``, an abbreviation or acronym is its text, not ``<abbr>``, and a code block's ``:number-lines:`` numbers no lines.

The page is written as `Markdown <../writing/markdown.md>`__ is, so `Styling Markdown <../themes/styling-markdown.md>`__ shows the HTML it is, for a theme's CSS.
